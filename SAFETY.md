# Safety

CyberEpi is a research and teaching simulator for cyber epidemiology.

Allowed:

- mathematical compartments and graph transitions
- synthetic organisations and synthetic pathogen profiles
- read-only Kubernetes metadata, excluding secret values
- comparison of defensive controls as parameter changes

Not part of this project, and not implemented:

- exploit payloads, scanners, or proof-of-concept attack code
- credential theft, persistence, or lateral-movement tooling
- destructive actions against data or infrastructure
- instructions for compromising a real system

If a change would make the simulator able to affect a real environment, it does not belong here.

Kubernetes discovery uses a ClusterRole limited to `get`, `list`, and `watch`. It does not grant `create`, `update`, `patch`, `delete`, `bind`, `escalate`, or `impersonate`. Secret imports must use partial object metadata. The snapshot type in `epi-kubernetes` has name, namespace, and secret type only.
