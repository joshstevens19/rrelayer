FROM rust:trixie AS builder
ARG TARGETPLATFORM
ARG BINARY_PATH
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

# Install build dependencies (needed for both build-from-source and ca-certificates)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy source code (needed for ca-certificates and backward compatibility)
COPY . .

# Copy docker-binary directory (created by workflow when using pre-built binary)
# The workflow ensures this directory exists; if building from source directly, it may be empty
COPY docker-binary/ /tmp/docker-binary/

# If pre-built binary exists, use it; otherwise build from source
RUN if [ -f /tmp/docker-binary/rrelayer_cli ]; then \
        echo "Using pre-built binary"; \
        mkdir -p /app/target/release; \
        cp /tmp/docker-binary/rrelayer_cli /app/target/release/rrelayer_cli; \
        chmod +x /app/target/release/rrelayer_cli; \
    else \
        echo "Building from source"; \
        if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
            RUSTFLAGS='-C target-cpu=x86-64-v2' cargo build --release --features jemalloc --workspace --exclude rust-sdk-playground --exclude e2e-tests; \
        else \
            RUSTFLAGS="-C target-cpu=neoverse-n1" cargo build --release --workspace --exclude rust-sdk-playground --exclude e2e-tests; \
        fi; \
    fi

FROM debian:trixie-slim
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