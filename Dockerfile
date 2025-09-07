# Multi-stage build for QNet
FROM rust:alpine as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:latest
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/echo /usr/local/bin/qnet-echo
ENTRYPOINT ["/bin/bash","-lc","echo 'QNet container ready'; sleep infinity"]
