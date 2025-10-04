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

# Build the neems-data binary using nightly toolchain
RUN cargo +nightly build --release --bin neems-data

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
COPY --from=builder /usr/src/app/target/release/neems-data .

# Create directory for shared data (SQLite database) with write permissions
RUN mkdir -p /app/data && \
    chmod 777 /app/data

# Set environment variables
ENV SITE_DATABASE_URL=/app/data/site-data.sqlite

# Run the application
CMD ["./neems-data", "monitor", "--verbose"]
