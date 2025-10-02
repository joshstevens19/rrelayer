FROM --platform=linux/amd64 rust:1.88.0-bookworm as builder
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build for standard Linux (glibc) instead of musl
RUN RUSTFLAGS='-C target-cpu=x86-64-v2' cargo build --release --features jemalloc --workspace --exclude rust-sdk-playground --exclude e2e-tests

FROM --platform=linux/amd64 debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rrelayer_cli /app/rrelayer
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ENTRYPOINT ["/app/rrelayer"]
