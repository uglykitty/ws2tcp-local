FROM rust:1-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release --locked

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source="https://github.com/uglykitty/ws2tcp-local"

COPY --from=builder /app/target/release/ws2tcp-local /usr/local/bin/ws2tcp-local

USER 10001:10001
EXPOSE 8000

ENV RUST_LOG=ws2tcp_local=info

ENTRYPOINT ["ws2tcp-local"]
