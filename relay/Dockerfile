FROM rust:1-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .
RUN cargo build --release -p tethera-relay

FROM debian:stable-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/tethera-relay /usr/local/bin/tethera-relay

EXPOSE 8080 443

ENTRYPOINT ["tethera-relay", "run", "--config", "/etc/tethera/relay.toml"]
