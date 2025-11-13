FROM rust:1.88.0-bookworm AS builder
ARG TARGETPLATFORM
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
# Use architecture-specific target triples and CPU optimization flags
RUN if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
        RUSTFLAGS='-C target-cpu=x86-64-v2' cargo build --release --features jemalloc --workspace --exclude rust-sdk-playground --exclude e2e-tests; \
    else \
        cargo build --release --workspace --exclude rust-sdk-playground --exclude e2e-tests; \
    fi

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rrelayer_cli /app/rrelayer
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app/project

ENTRYPOINT ["/app/rrelayer"]
