# syntax=docker/dockerfile:1
FROM rust:1.97.1-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY assets ./assets
COPY src ./src
RUN cargo build --locked --release \
    && mkdir /build/data

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
WORKDIR /app
COPY --from=builder /build/target/release/inp-calendar-bot /usr/local/bin/inp-calendar-bot
COPY --from=builder --chown=65532:65532 /build/data /app/data
ENV DATA_DIR=/app/data TZ=Europe/Paris
VOLUME ["/app/data"]
ENTRYPOINT ["/usr/local/bin/inp-calendar-bot"]
