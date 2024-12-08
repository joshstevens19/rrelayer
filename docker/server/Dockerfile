# Use an official Rust image as a parent image
FROM rust:latest as builder

# Create a new empty shell project
RUN USER=root cargo new --bin app
WORKDIR /app

# Copy our manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Build only the dependencies to cache them
# RUN cargo build --profile maxperf --features jemalloc
RUN cargo build
RUN rm src/*.rs

# Now that the dependencies are built, copy your source code
COPY ./src ./src

# Build your application prod
# RUN rm ./target/maxperf/deps/rrelayerr_server*
# RUN cargo build --profile maxperf --features jemalloc

RUN rm ./target/debug/deps/rrelayerr_server*
RUN cargo build
# Consider adding a RUN ls /app/target/maxperf/ here to confirm the binary's location

# Use Debian bullseye-slim as the base image for the final stage
FROM debian:bullseye-slim
ARG APP=/usr/src/app

# Install OpenSSL and clean up in one layer
RUN apt-get update \
    && apt-get install -y ca-certificates tzdata openssl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage PROD
# COPY --from=builder /app/target/maxperf/rrelayerr_server ${APP}/rrelayerr_server

COPY --from=builder /app/target/debug/rrelayerr_server ${APP}/rrelayerr_server

# Ensure the binary is executable
RUN chmod +x ${APP}/rrelayerr_server

# Set the working directory and the binary as the entrypoint
WORKDIR ${APP}
ENTRYPOINT ["./rrelayerr_server"]
