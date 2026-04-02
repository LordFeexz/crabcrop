# ── Stage 1: Build libvips from source ──────────────────────────────────────
FROM debian:bookworm-slim AS vips-builder

ARG VIPS_VERSION=8.16.1

RUN apt-get update && apt-get install -y \
    build-essential \
    meson \
    ninja-build \
    pkg-config \
    wget \
    # Core image format libs
    libglib2.0-dev \
    libexpat1-dev \
    libjpeg62-turbo-dev \
    libpng-dev \
    libwebp-dev \
    libtiff-dev \
    libexif-dev \
    libgsf-1-dev \
    # HEIF/AVIF codec stack (must be installed BEFORE libvips is compiled)
    libheif-dev \
    libaom-dev \
    libde265-dev \
    libx265-dev \
    # GIF, SVG, ICC
    libcgif-dev \
    librsvg2-dev \
    liblcms2-dev \
    # ImageMagick compatibility layer
    libmagickcore-dev \
    libmagickwand-dev \
    && rm -rf /var/lib/apt/lists/*

# Download and compile libvips with ALL codecs
RUN wget -qO- "https://github.com/libvips/libvips/releases/download/v${VIPS_VERSION}/vips-${VIPS_VERSION}.tar.xz" \
    | tar -xJ -C /tmp \
    && cd /tmp/vips-${VIPS_VERSION} \
    && meson setup build --prefix=/usr/local --buildtype=release --strip \
    && ninja -C build \
    && ninja -C build install \
    && ldconfig

# ── Stage 2: Build Rust binary ───────────────────────────────────────────────
FROM rust:1.94.1-slim-bookworm AS builder

# Install only what the Rust linker needs — use the custom-built libvips from Stage 1
COPY --from=vips-builder /usr/local /usr/local

RUN apt-get update && apt-get install -y \
    pkg-config \
    clang \
    build-essential \
    libglib2.0-dev \
    libexpat1-dev \
    libjpeg62-turbo-dev \
    libpng-dev \
    libwebp-dev \
    libtiff-dev \
    libexif-dev \
    libgsf-1-dev \
    libheif-dev \
    libaom-dev \
    libde265-dev \
    libcairo2-dev \
    liblcms2-dev \
    libopenjp2-7-dev \
    libopenexr-dev \
    librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

RUN ldconfig

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY .cargo ./.cargo

RUN cargo build --release

# ── Stage 3: Minimal runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim

# Install ONLY runtime libs (no -dev packages needed)
RUN apt-get update && apt-get install -y \
    libglib2.0-0 \
    libjpeg62-turbo \
    libpng16-16 \
    libwebp7 \
    libwebpmux3 \
    libwebpdemux2 \
    libtiff6 \
    libexif12 \
    libgsf-1-114 \
    libheif1 \
    libaom3 \
    libde265-0 \
    libx265-199 \
    libcgif0 \
    librsvg2-2 \
    liblcms2-2 \
    libcairo2 \
    libopenjp2-7 \
    libopenexr-3-1-30 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the custom-built libvips .so files into the runtime image
COPY --from=vips-builder /usr/local/lib /usr/local/lib
COPY --from=vips-builder /usr/local/bin/vips /usr/local/bin/vips
RUN ldconfig

COPY --from=builder /usr/src/app/target/release/crabcrop /app/crabcrop

WORKDIR /app
RUN mkdir -p .cache/images && chmod 777 .cache/images

EXPOSE 3005
ENV PORT=3005
ENV RUST_LOG=crabcrop=info,tower_http=info

CMD ["./crabcrop"]
