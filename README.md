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
- **Click tracking** *(To Do)* — recording metadata via background workers and `mpsc` channels with batched DB inserts
- **Rate limiting** *(To Do)* — protecting endpoints from abuse using IP-based tracking

### Phase 3 & Beyond (Future Work)
- Distributed ID generation (Snowflake IDs)
- Horizontal scaling with read replicas
- Redis clustering
- Observability — metrics, Prometheus/Grafana

## Tech Stack

- **[Axum](https://github.com/tokio-rs/axum)** — async web framework
- **[SQLx](https://github.com/launchbadge/sqlx)** — async PostgreSQL driver with compile-time query checking
- **[Fred](https://github.com/aembke/fred.rs)** — async Redis client built natively for Tokio
- **[Tokio](https://tokio.rs)** — async runtime
- **[Tracing](https://github.com/tokio-rs/tracing)** — structured logging and diagnostics
- **[Serde](https://serde.rs)** — serialization/deserialization
- **[dotenvy](https://github.com/allan2/dotenvy)** — environment variable management
- **[time](https://crates.io/crates/time)** (or **chrono**) — date and time handling for link expiry and click tracking
- **[axum-extra](https://crates.io/crates/axum-extra)** / **[axum-client-ip](https://crates.io/crates/axum-client-ip)** — secure extraction of client IP addresses and headers

## Project Structure

```text
src/
├── main.rs          # startup, router assembly, background worker spawning
├── state.rs         # AppState definition (includes mpsc sender)
├── errors.rs        # AppError, IntoResponse impl
├── models.rs        # Data structures (FetchedLink, ClickEvent)
├── encode.rs        # Base62 encode/decode
├── db.rs            # all database queries
├── cache.rs         # Redis get/set operations
├── redis.rs         # Redis client setup
├── workers.rs       # Background tasks (e.g., click tracking batcher)
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
git clone https://github.com/yourname/snip-rs
cd snip-rs
```

2. Create a `.env` file:
```env
DATABASE_URL=postgres://username:password@localhost:5432/snip
REDIS_URL=redis://localhost:6379/
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
| p75 latency | 6.02ms |
| p90 latency | 6.42ms |
| p99 latency | 7.23ms |

### Phase 2 — Without Redis cache (1000 connections)

| Metric | Value |
|---|---|
| Requests/sec | 17,439 |
| p50 latency | 57.92ms |
| p75 latency | 59.75ms |
| p90 latency | 61.22ms |
| p99 latency | 69.62ms |

### Phase 2 — With Redis cache (1000 connections)

| Metric | Value |
|---|---|
| Requests/sec | 63,844 |
| p50 latency | 14.94ms |
| p75 latency | 16.57ms |
| p90 latency | 19.05ms |
| p99 latency | 31.58ms |

**3.6x throughput increase and 3.9x p50 latency reduction** after adding Redis caching. **The bottleneck without cache is the Postgres connection pool** — under 1000 concurrent connections, requests queue for a DB connection. With Redis, the vast majority of redirect lookups never touch the database.

## Architecture

```text
Client → Axum → Redis (cache hit → redirect)
                     ↓ cache miss
                  Postgres → cache (with dynamic TTL) → redirect
```

On a cache miss, the result is written back to Redis with a **dynamic TTL (the minimum of 1 week or the link's actual expiration time)** to prevent serving expired links from the cache. Redis uses an `allkeys-lru` eviction policy so the most recently active links stay warm and cold links fall out naturally.

If Redis is unavailable, the app degrades gracefully — all requests fall through to Postgres without error.

## License

MIT
