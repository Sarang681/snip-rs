# snip-rs

A high-performance URL shortener built with Rust, Axum, PostgreSQL, and Redis. Designed as a learning project with a clear progression from a simple single-server implementation toward a distributed system capable of handling millions of requests.

## Features

### Phase 1 — Core URL Shortener
- `POST /shorten` — accepts a long URL, returns a Base62 short code
- `GET /:code` — redirects to the original URL
- Base62 encoding for compact, readable short codes
- Input validation — rejects malformed URLs before hitting the database
- Structured error handling with appropriate HTTP status codes

### Phase 2 — Production Hardening
- **Structured logging** via `tracing` and `tower-http` TraceLayer
- **Redis caching** — redirect lookups served from memory on cache hits, graceful degradation if Redis is unavailable
- **Link expiry** — optional TTL on short codes, returns `410 Gone` for expired links, with dynamic Redis cache TTLs
- **Click tracking pipeline** — non-blocking, load-shedding MPSC channel decouples redirects from DB writes. A background worker batches inserts based on time (1s) and size (500) thresholds.
- **Rate limiting** — custom Axum extractor protecting `POST /shorten`. Uses a Redis Fixed Window algorithm with a local `moka` in-memory fallback for graceful degradation if Redis goes down.

### Phase 3 & Beyond (Future Work)
- Containerization (Docker) and local orchestration (Docker Compose)
- CI/CD pipelines (GitHub Actions)
- Distributed ID generation (Snowflake IDs)
- Horizontal scaling with read replicas
- Redis clustering
- Observability — metrics, Prometheus/Grafana

## Tech Stack

- **[Axum](https://github.com/tokio-rs/axum)** — async web framework
- **[SQLx](https://github.com/launchbadge/sqlx)** — async PostgreSQL driver with compile-time query checking
- **[Fred](https://github.com/aembke/fred.rs)** — async Redis client built natively for Tokio
- **[Tokio](https://tokio.rs)** — async runtime
- **[Moka](https://github.com/moka-rs/moka)** — high-performance, concurrent in-memory cache (used for rate limit fallback)
- **[Tracing](https://github.com/tokio-rs/tracing)** — structured logging and diagnostics
- **[Serde](https://serde.rs)** — serialization/deserialization
- **[dotenvy](https://github.com/allan2/dotenvy)** — environment variable management
- **[time](https://crates.io/crates/time)** — date and time handling for link expiry and click tracking

## Project Structure

```text
src/
├── main.rs          # startup, router assembly, background worker spawning
├── state.rs         # AppState definition (includes mpsc sender, moka cache)
├── errors.rs        # AppError, IntoResponse impl
├── models.rs        # Data structures (FetchedLink, ClickEvent, ShortenRequest)
├── encode.rs        # Base62 encode/decode
├── db.rs            # all database queries (including batch inserts)
├── redis.rs         # Redis client setup and get/set/incr operations
├── receiver.rs      # Background worker (mpsc receiver, tokio::select! batching)
├── extractors/      # Custom Axum extractors (e.g., RateLimited)
└── routes/
    ├── mod.rs       # merges all routers
    └── urls.rs      # POST /shorten, GET /:code handlers
```

## Getting Started

### Prerequisites

- Rust (stable)
- PostgreSQL
- Redis
- [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)

### Setup

1. Clone the repository:
```bash
git clone https://github.com/Sarang681/snip-rs
cd snip-rs
```

2. Create a `.env` file:
```env
DATABASE_URL=postgres://username:password@localhost:5432/snip
REDIS_URL=redis://localhost:6379/
RUST_LOG=info,tower_http=debug
```

3. Run database migrations:
```bash
sqlx migrate run
```

4. Build and run:
```bash
cargo run --release
```

### Usage

Shorten a URL:
```bash
curl -X POST http://localhost:8080/shorten \
  -H "Content-Type: application/json" \
  -d '{"long_url": "https://example.com/some/very/long/url"}'
```

Response:
```json
{ "short_code": "3Jt" }
```

Follow a short code (redirects to original URL):
```bash
# Use -L to follow the redirect, or -I to see the 307 Temporary Redirect headers without following it
curl -L http://localhost:8080/3Jt
```

## Performance

Benchmarked with [wrk](https://github.com/wg/wrk) — 4 threads, 1000 concurrent connections, 30 second duration, on a single machine running the app, Postgres, and Redis locally.

### Phase 1 — Postgres only (100 connections)

| Metric | Value |
|---|---|
| Requests/sec | 17,780 |
| p50 latency | 5.59ms |
| p99 latency | 7.23ms |

### Phase 2 — With Redis cache (1000 connections)

| Metric | Value |
|---|---|
| Requests/sec | 63,844 |
| p50 latency | 14.94ms |
| p99 latency | 31.58ms |

### Phase 2 — Fully Loaded (Analytics + Rate Limiting + Load Shedding)

| Metric | Value |
|---|---|
| Requests/sec | **41,830** |
| p50 latency | **23.21ms** |
| p99 latency | **41.16ms** |

**Note:** Even with the added overhead of extracting headers, allocating click events, and checking the rate limiter on every request, the system maintains a blazing fast ~23ms average latency. The slight drop in total throughput compared to the pure Redis cache benchmark is the intentional cost of capturing analytics and enforcing security.

## Performance & Architectural Trade-offs: Click Tracking

Building a high-throughput analytics pipeline for a URL shortener presents a classic distributed systems challenge: **how to record telemetry without degrading the primary user experience (the redirect).**

To solve this, `snip-rs` uses an asynchronous, bounded MPSC channel to decouple the HTTP redirect handler from the database write operation. During load testing (`wrk -t4 -c1000 -d30s`), we evaluated two distinct backpressure strategies:

| Strategy | Implementation | Throughput | P99 Latency | Data Retention | Behavior Under Load |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Strict Backpressure** | `.send().await` | ~6,000 req/sec | ~795 ms | **100%** | HTTP handlers pause and wait for the DB when the channel fills. Guarantees zero data loss, but severely degrades user-facing latency during traffic spikes. |
| **Load Shedding** *(Chosen)* | `.try_send()` | **~41,800 req/sec** | **~41 ms** | **~25%** *(during extreme sustained spikes)* | If the 100,000-capacity channel fills, analytics events are instantly dropped. The redirect is returned immediately. |

### Why "Load Shedding" was chosen:
For a URL shortener, **redirect latency is mission-critical**, while analytics are "nice-to-have." 

By combining a large memory buffer (100,000 events, consuming <30MB of RAM) with non-blocking `.try_send()`, the system acts as a shock absorber. It perfectly captures sudden, short traffic spikes (like a link going viral) with zero data loss. However, during sustained, extreme load (e.g., a DDoS attack or massive viral event), it gracefully sheds telemetry load to guarantee that **every single user experiences a sub-50ms redirect**, while strictly capping memory usage to prevent Out-Of-Memory crashes. 

## Security & Fault Tolerance: Rate Limiting

The `POST /shorten` endpoint is protected by a custom Axum extractor that enforces a strict Fixed Window rate limit (e.g., 5 requests per minute per IP). 

**Defense in Depth (Graceful Degradation):**
If the primary Redis instance goes down or experiences high latency, the application **does not fail closed** with a 500 error. Instead, it seamlessly falls back to a local, concurrent `moka` in-memory cache. 
- This ensures that a single malicious IP cannot overwhelm the local server's CPU or database pool, even during a catastrophic infrastructure failure.
- The local cache is configured with a 60-second TTL to perfectly mirror the Redis window and automatically clean up memory.

## Architecture

```text
Client -> Axum Router
          |
          +-> [Rate Limiter Extractor] -> Redis (Primary) / Moka (Fallback)
          |
          +-> POST /shorten -> Postgres (Write)
          |
          +-> GET /:code -> Redis (Cache Hit? -> Redirect)
                             | (Cache Miss)
                             +-> Postgres -> Redis (Dynamic TTL) -> Redirect
                             |
                             +-> MPSC Channel -> Background Worker -> Postgres (Batch Insert Clicks)
```

On a cache miss, the result is written back to Redis with a **dynamic TTL (the minimum of 1 week or the link's actual expiration time)** to prevent serving expired links from the cache. Redis uses an `allkeys-lru` eviction policy so the most recently active links stay warm and cold links fall out naturally.

If Redis is unavailable, the app degrades gracefully — all requests fall through to Postgres without error, and rate limiting falls back to local memory.

## License

MIT
