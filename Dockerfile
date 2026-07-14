FROM rust:1-slim-bookworm AS builder

WORKDIR /app/ws2tcp-local

COPY ws2tcp-local-core /app/ws2tcp-local-core
COPY ws2tcp-local/Cargo.toml ws2tcp-local/Cargo.lock ./
COPY ws2tcp-local/src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source="https://github.com/uglykitty/ws2tcp-local"

COPY --from=builder /app/ws2tcp-local/target/release/ws2tcp-local /usr/local/bin/ws2tcp-local

USER 10001:10001
EXPOSE 3128

VOLUME ["/etc/ws2tcp-local"]

ENV RUST_LOG=ws2tcp_local=info

ENTRYPOINT ["ws2tcp-local"]
CMD ["--config", "/etc/ws2tcp-local/ws2tcp-local.toml"]
