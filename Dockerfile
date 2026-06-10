# crates/sync — Contextful backend, containerized for the ECS Fargate demo
# (infra/ecs). Build context is the repo root; only the Cargo workspace is
# copied in. rusqlite is `bundled` and TLS is rustls, so the runtime image
# needs nothing beyond ca-certificates.

FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --release -p sync

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/sync /usr/local/bin/sync
COPY infra/ecs/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

# Fargate tasks are ephemeral — state lives inside the task's filesystem and
# resets on redeploy, which is exactly right for a throwaway demo relay.
ENV CONTEXTFUL_HOME=/data
RUN mkdir -p /data && chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 7878 7979
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["serve", "--addr", "0.0.0.0:7878", "--with-mcp", "--mcp-addr", "0.0.0.0:7979"]
