FROM rust:1-alpine AS builder

ARG WS2TCP_LOCAL_VERSION=0.1.7

RUN cargo install \
        --locked \
        --version "${WS2TCP_LOCAL_VERSION}" \
        ws2tcp-local

FROM alpine:latest

LABEL org.opencontainers.image.source="https://github.com/uglykitty/ws2tcp-local"

RUN apk add --no-cache ca-certificates

COPY --from=builder /usr/local/cargo/bin/ws2tcp-local /usr/local/bin/ws2tcp-local

USER 10001:10001
EXPOSE 8000

ENV RUST_LOG=ws2tcp_local=info

ENTRYPOINT ["ws2tcp-local"]
