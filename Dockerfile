# Builder stage
FROM rust:1.80-slim-bookworm AS builder

# Install build dependencies for libvips
RUN apt-get update && apt-get install -y \
    pkg-config \
    libvips-dev \
    clang \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy the complete source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY .cargo ./.cargo

# Build the release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies for libvips and CA certificates for reqwest/HTTPS
RUN apt-get update && apt-get install -y \
    libvips42 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/crabcrop .

# Ensure the disk cache directory exists and is writable
RUN mkdir -p .cache/images && chmod 777 .cache/images

# Expose the default port
EXPOSE 3005

# Operational environment variables
ENV PORT=3005
ENV RUST_LOG=crabcrop=info,tower_http=info

# Run the binary
CMD ["./crabcrop"]
