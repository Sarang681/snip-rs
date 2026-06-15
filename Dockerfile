FROM rust:1.96.0-bookworm AS builder

# Install build dependencies required by crates
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  libpq-dev \
  && rm -rf /var/lib/apt/lists/*

# Set the working directory inside the container
WORKDIR /app

#Install sqlx cli
RUN cargo install sqlx-cli --no-default-features --features postgres

# Copy the manifest files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY .sqlx ./.sqlx
COPY migrations ./migrations

# Build the application in release mode
ENV SQLX_OFFLINE=true
RUN cargo build --release

## Stage 2
FROM debian:bookworm-slim AS runtime

# Install the dependencies needed to run the binary
RUN apt-get update && apt-get install -y \
  libssl3 \
  libpq5 \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Copy the sqlx binary
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy the binary from build stage
COPY --from=builder /app/target/release/snip-rs .

COPY --from=builder /app/migrations ./migrations

COPY ./entrypoint.sh ./entrypoint.sh
RUN chmod +x ./entrypoint.sh

# Create non-root user to run the application
RUN useradd -m -u 1000 appuser
USER appuser

ENV DATABASE_URL=postgres://snip_user:snip_password@db:5432/snip_db

# EXPOSE port 8080
EXPOSE 8080

# Run the application
ENTRYPOINT ["./entrypoint.sh"]
