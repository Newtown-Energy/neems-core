# Use the official Rust nightly image as the base
FROM rustlang/rust:nightly-bookworm as builder

# Install sqlite3
RUN apt-get update && apt-get install -y \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /usr/src/app

# Copy the entire workspace
COPY . .

# Build the neems-api binary using nightly toolchain
RUN cargo +nightly build --release --bin neems-api

# Create a runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    sqlite3 \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/release/neems-api .

# Copy configuration files
COPY --from=builder /usr/src/app/Rocket.toml .

# Create directory for databases with write permissions
RUN mkdir -p /app/data && \
    chmod 777 /app/data

# Create directory for static files (if needed)
RUN mkdir -p /app/static && \
    chmod 755 /app/static

# Set environment variables
ENV DATABASE_URL=/app/data/neems-api.db
ENV SITE_DATABASE_URL=/app/data/site-data.sqlite
ENV NEEMS_STATIC_DIR=/app/static

# Expose the default Rocket port
EXPOSE 8000

# Run the application
CMD ["./neems-api"]
