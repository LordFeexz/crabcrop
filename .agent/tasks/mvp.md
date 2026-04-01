# 🦀 Mini CDN Image Processing (Rust) — Task Breakdown

---

## 📦 EPIC 1 — Project Setup

### TASK 1.1 — Initialize Project
- [ ] Create new Rust project
- [ ] Setup workspace structure
- [ ] Add base dependencies:
  - axum
  - tokio
  - serde
  - tracing

### TASK 1.2 — Basic Server
- [ ] Create HTTP server using Axum
- [ ] Implement `/health` endpoint
- [ ] Add logging middleware (tracing)

---

## 🖼️ EPIC 2 — Image Processing Core

### TASK 2.1 — Setup libvips
- [ ] Install libvips on system
- [ ] Add Rust binding (vips crate)
- [ ] Verify installation with sample image

### TASK 2.2 — Image Loader
- [ ] Load image from local file
- [ ] Support buffer input (Vec<u8>)
- [ ] Validate image format

### TASK 2.3 — Resize Function
- [ ] Implement resize (width, height)
- [ ] Support aspect ratio
- [ ] Add fit modes:
  - cover
  - contain
  - fill

### TASK 2.4 — Format Conversion
- [ ] Convert to WebP
- [ ] Convert to AVIF (optional)
- [ ] Keep original format fallback

### TASK 2.5 — Quality Control
- [ ] Add quality parameter
- [ ] Optimize compression

---

## 🌐 EPIC 3 — HTTP Image Endpoint

### TASK 3.1 — Route Setup
- [ ] Create `/img` endpoint
- [ ] Support GET method

### TASK 3.2 — Query Parser
- [ ] Parse parameters:
  - url
  - w
  - h
  - format
  - q
- [ ] Validate input

### TASK 3.3 — Image Fetcher
- [ ] Fetch image via HTTP
- [ ] Handle timeout
- [ ] Validate response (content-type)

### TASK 3.4 — Response Builder
- [ ] Return processed image
- [ ] Set correct headers:
  - Content-Type
  - Cache-Control

---

## ⚡ EPIC 4 — Cache System

### TASK 4.1 — Cache Key Generator
- [ ] Implement hash function (blake3)
- [ ] Generate unique key:


### TASK 4.2 — Memory Cache
- [ ] Integrate moka cache
- [ ] Store processed images (bytes)
- [ ] Configure TTL

### TASK 4.3 — Disk Cache
- [ ] Create cache directory
- [ ] Save processed images
- [ ] Load image from disk if exists

### TASK 4.4 — Cache Flow Integration
- [ ] Check memory cache
- [ ] Check disk cache
- [ ] Fallback to processing
- [ ] Save to cache

---

## 🔁 EPIC 5 — Request Deduplication

### TASK 5.1 — Dedup Manager
- [ ] Create shared map for in-flight requests
- [ ] Key: cache key

### TASK 5.2 — Singleflight Logic
- [ ] If request exists → wait result
- [ ] If not → process and store future

### TASK 5.3 — Cleanup
- [ ] Remove entry after processing complete

---

## ☁️ EPIC 6 — Storage Integration (DO Spaces)

### TASK 6.1 — S3 Client Setup
- [ ] Configure aws-sdk-s3
- [ ] Setup credentials
- [ ] Test connection

### TASK 6.2 — Image Fetch from Storage
- [ ] Load image from bucket
- [ ] Support public/private access

---

## ⚙️ EPIC 7 — Performance Optimization

### TASK 7.1 — Concurrency Control
- [ ] Implement semaphore
- [ ] Limit max concurrent processing

### TASK 7.2 — Timeout Handling
- [ ] Set timeout for external fetch
- [ ] Handle failure gracefully

### TASK 7.3 — Streaming Optimization
- [ ] Avoid full memory load
- [ ] Use libvips streaming

---

## 🧠 EPIC 8 — Smart Features

### TASK 8.1 — Auto Format Detection
- [ ] Detect Accept header
- [ ] Choose best format:
- AVIF
- WebP
- JPEG

### TASK 8.2 — Metadata Optimization
- [ ] Strip EXIF metadata

### TASK 8.3 — Default Optimization
- [ ] Progressive encoding
- [ ] Sensible defaults

---

## 📦 EPIC 9 — Deployment

### TASK 9.1 — Build Binary
- [ ] Build release version
- [ ] Optimize binary size

### TASK 9.2 — Server Setup
- [ ] Setup DigitalOcean droplet
- [ ] Install dependencies (libvips)

### TASK 9.3 — Service Setup
- [ ] Create systemd service
- [ ] Enable auto restart

### TASK 9.4 — Reverse Proxy (Optional)
- [ ] Setup nginx
- [ ] Enable gzip/brotli

---

## 📊 EPIC 10 — Testing & Benchmark

### TASK 10.1 — Load Testing
- [ ] Setup k6 or wrk
- [ ] Test concurrency

### TASK 10.2 — Performance Metrics
- [ ] Measure latency
- [ ] Measure throughput

### TASK 10.3 — Cache Testing
- [ ] Hit vs miss ratio
- [ ] Validate caching works

---

## 🧪 EPIC 11 — Error Handling & Edge Cases

### TASK 11.1 — Invalid URL
- [ ] Return 400 error

### TASK 11.2 — Image Not Found
- [ ] Return 404

### TASK 11.3 — Processing Error
- [ ] Return fallback / error image

---

## 🔐 EPIC 12 — Security (Optional Advanced)

### TASK 12.1 — Signed URL
- [ ] Add signature validation
- [ ] Prevent abuse

### TASK 12.2 — Rate Limiting
- [ ] Limit per IP

---

## 🧩 AI Agent Execution Notes

### Execution Strategy
1. Complete EPIC sequentially
2. Validate each step before moving forward
3. Write test for critical components:
 - processor
 - cache
 - deduplication

### Coding Guidelines
- Use async/await (tokio)
- Avoid blocking operations
- Prefer streaming over buffering
- Always check cache first

---

## 🎯 Definition of Done (MVP)

- [ ] Can process image via URL
- [ ] Cache working (memory + disk)
- [ ] Handles 100+ concurrent requests
- [ ] No duplicate processing
- [ ] Deployable on DigitalOcean

---

## 🚀 Stretch Goals

- [ ] Redis distributed cache
- [ ] Multi-node scaling
- [ ] Web dashboard monitoring
- [ ] Preload popular images