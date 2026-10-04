# CyberEpi controller, worker, and CLI.
# Build: docker build -t cyberepi:local .
# The same image runs either process; the container command selects it.

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY cli cli
COPY services services
COPY scenarios scenarios
COPY schemas schemas
COPY docs docs
RUN cargo build --locked --release -p cyberepi-controller -p cyberepi-worker -p cyberepi

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 65532 --create-home cyberepi
COPY --from=build /src/target/release/cyberepi-controller /usr/local/bin/cyberepi-controller
COPY --from=build /src/target/release/cyberepi-worker /usr/local/bin/cyberepi-worker
COPY --from=build /src/target/release/cyberepi /usr/local/bin/cyberepi
USER 65532:65532
EXPOSE 8080 9102
ENTRYPOINT ["cyberepi-controller"]
