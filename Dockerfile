# Multi-stage build for QNet
FROM rust:1.81 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:stable-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/echo /usr/local/bin/qnet-echo || true
ENTRYPOINT ["/bin/bash","-lc","echo 'QNet container ready'; sleep infinity"]
