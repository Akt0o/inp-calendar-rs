# syntax=docker/dockerfile:1
FROM rust:1.87-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY assets ./assets
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/target/release/inp-calendar-bot /usr/local/bin/inp-calendar-bot
COPY --from=builder /build/assets ./assets
ENV DATA_DIR=/app/data TZ=Europe/Paris
VOLUME ["/app/data"]
ENTRYPOINT ["inp-calendar-bot"]
