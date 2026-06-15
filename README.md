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

### Phase 3 — Containerization & Orchestration
- **Multi-stage Docker builds** — tiny, secure runtime images (~30MB) without the Rust compiler baggage.
- **Automated Migrations** — `entrypoint.sh` script guarantees database schema is applied before the app starts.
- **One-Command Local Dev** — `podman-compose` / `docker-compose` orchestrates the App, PostgreSQL, and Redis on an isolated internal network with health checks.

### Phase 4 & Beyond (Future Work)
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
- **[Docker / Podman](https://www.docker.com/)** — containerization and local orchestration

## Project Structure

```text
src/
├── main.rs          # startup, router assembly, background worker spawning
├── state.rs         # AppState definition (includes mpsc sender, moka cache)
├── errors.rs        # AppError, IntoResponse impl
├── models.rs        # Data structures (FetchedLink, ClickEvent, ShortenRequest)
├── encode.rs        # Base62 encode/decode
├── db.rs            # all database queries (including batch inserts)
├── redis.rs         # Redis client setup & rate limit logic
├── receiver.rs      # Background worker (mpsc receiver, tokio::select! batching)
├── extractors/      # Custom Axum extractors (e.g., RateLimited)
└── routes/
    ├── mod.rs       # merges all routers
    └── urls.rs      # POST /shorten, GET /:code handlers
```

## Getting Started

### Option A: Containerized Setup (Recommended)
This is the fastest way to get the entire stack (App, Postgres, Redis) running locally with zero manual database setup.

**Prerequisites:** [Docker](https://www.docker.com/) or [Podman](https://podman.io/) with `podman-compose` or `docker-compose`.

1. Clone the repository:
```bash
git clone https://github.com/Sarang681/snip-rs
cd snip-rs
```

2. **Crucial Step:** Generate the SQLx offline cache. This allows the Docker builder to verify queries without a live database. *(Requires a local Postgres instance just for this step, or you can skip if the `.sqlx` folder is already committed).*
```bash
cargo sqlx prepare
```

3. Ensure your `.env` file is configured for the container network (using `db` and `redis` as hostnames):
```env
DATABASE_URL=postgres://snip_user:snip_password@db:5432/snip_db
REDIS_URL=redis://redis:6379/
RUST_LOG=info,tower_http=debug
```

4. Spin up the entire stack with a single command. This will build the multi-stage image, wait for Postgres/Redis health checks, run migrations automatically, and start the app:
```bash
# For Docker users:
docker-compose up --build

# For Podman users:
podman-compose up --build
```

### Option B: Local Host Setup
If you prefer to run the Rust binary directly on your host machine.

**Prerequisites:** Rust (stable), PostgreSQL, Redis, [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)

1. Create a `.env` file pointing to your local services:
```env
DATABASE_URL=postgres://username:password@localhost:5432/snip
REDIS_URL=redis://localhost:6379/
RUST_LOG=info,tower_http=debug
```

2. Run database migrations:
```bash
sqlx migrate run
```

3. Build and run:
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
# Use -L to follow the redirect, or -I to see the 307 Temporary Redirect headers
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

**Note:** Even with the added overhead of extracting headers, allocating click events, and checking the rate limiter on every request, the system maintains a blazing fast ~23ms average latency. 

## Architectural Decisions & Trade-offs

Building a production-grade system requires making deliberate choices based on the specific constraints of the application. Below is a detailed breakdown of the architectural trade-offs encountered in `snip-rs`.

### 1. Telemetry vs. User Latency (Click Tracking)
**The Decision:** We use a non-blocking, load-shedding MPSC channel (`.try_send()`) paired with a massive 100,000-item memory buffer, rather than strict blocking backpressure (`.send().await`).
**The Alternatives:** 
- *Strict Backpressure:* Pauses the HTTP handler if the DB is slow. Guarantees 100% data retention but spikes user-facing latency to ~800ms under load.
- *Unbounded Channel:* Never drops data, but risks an Out-Of-Memory (OOM) crash during a traffic spike.
**The "Why":** For a URL shortener, **redirect latency is mission-critical**, while analytics are "nice-to-have." By using a large but strictly bounded buffer (~30MB of RAM), the system acts as a shock absorber. It captures sudden, short traffic spikes with zero data loss. However, during sustained, extreme load, it gracefully sheds telemetry to guarantee that **every single user experiences a sub-50ms redirect**, while strictly capping memory usage to prevent OOM crashes.

### 2. Rate Limiting Algorithm (Fixed Window vs. Sliding Window)
**The Decision:** We implemented a Fixed Window algorithm using Redis `INCR` and `EXPIRE`.
**The Alternatives:** 
- *Sliding Window/Log:* Tracks exact timestamps of every request. Perfectly accurate, but requires heavy memory usage and complex Redis Sorted Sets.
**The "Why":** The Fixed Window approach uses exactly two ultra-fast Redis commands ($O(1)$ time complexity) and virtually zero memory. While it has a known "boundary burst" flaw, this is perfectly acceptable for a limit of 5 requests/minute. It effectively stops spam bots without wasting expensive Redis CPU cycles on perfect accuracy.

### 3. Framework Integration (Custom Extractor vs. Tower Middleware)
**The Decision:** We built the rate limiter as a custom Axum `FromRequestParts` Extractor rather than a Tower Middleware layer.
**The Alternatives:** 
- *Tower Middleware:* The traditional approach in many web frameworks. Intercepts the request at the network layer.
**The "Why":** Because `snip-rs` uses a centralized `AppError` enum to guarantee clean, standardized JSON error responses, using custom Tower middleware would require writing complex, verbose boilerplate to map low-level network errors back into our custom JSON format. A Custom Extractor integrates natively with Axum's handler-level error routing, requires 80% less code, and allows explicit, granular application to specific routes.

### 4. Infrastructure Resilience (Local Fallback vs. Fail-Closed)
**The Decision:** If Redis goes down, the rate limiter seamlessly falls back to a local, concurrent `moka` in-memory cache rather than returning a `500 Internal Server Error`.
**The Alternatives:** 
- *Fail-Closed:* Reject all requests if Redis is down. 
- *Fail-Open:* Allow all requests if Redis is down, disabling security entirely.
**The "Why":** This is the principle of **Defense in Depth**. If we fail-closed, a Redis blip effectively becomes a self-inflicted Denial of Service. By falling back to a local `moka` cache (configured with a 60s TTL), we guarantee that a single abusive IP cannot overwhelm the local server's CPU or database pool, even during a catastrophic infrastructure failure.

### 5. Fallback Concurrency (Best-Effort vs. Strict Atomicity)
**The Decision:** The local `moka` fallback uses a simple "read-then-write" approach rather than strict atomic counters.
**The "Why":** Because this is a *fallback* mechanism meant to survive infrastructure outages, "best-effort" is the right philosophy. The microscopic race condition (allowing 6 or 7 requests instead of strictly 5 under heavy concurrent load during an outage) is a perfectly acceptable trade-off for keeping the implementation simple, lock-free, and blazing fast.

### 6. Cache Eviction Strategy (Dynamic TTL vs. Static TTL)
**The Decision:** When writing to Redis on a cache miss, we set the TTL to the *minimum* of 1 week or the link's actual expiration time.
**The "Why":** A static TTL creates a dangerous edge case: if a link expires in 1 day, but stays in the Redis cache for 7 days, the app will serve a `307 Redirect` for an expired link, bypassing the database check. Dynamic TTL ensures expired links are instantly evicted from memory.

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

If Redis is unavailable, the app degrades gracefully — all requests fall through to Postgres without error, and rate limiting falls back to local memory.

## License

MIT
