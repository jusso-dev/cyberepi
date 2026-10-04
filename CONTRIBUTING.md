# Contributing

CyberEpi is for academic, educational, defensive, and research use.

- Keep propagation mathematical. Do not add exploit procedures.
- Mark new default rates as illustrative simulation assumptions.
- Keep the homelab install free of paid APIs and distributed storage.
- Add a test with the behaviour you change. Seeds must stay reproducible.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before opening a pull request.

The controller and worker share `epi-orchestrator`. The CLI calls the same scenario runner the workers use, so a local `cyberepi simulate` and a distributed batch stay on one engine.
