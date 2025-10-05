# Development Dockerfile for neems-api with live reload
FROM rustlang/rust:nightly-bookworm

# Install sqlite3 and cargo-watch for live reload
RUN apt-get update && apt-get install -y \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-watch

# Set working directory
WORKDIR /usr/src/app

# Create directory for databases with write permissions
RUN mkdir -p /app/data && \
    chmod 777 /app/data

# Create directory for static files
RUN mkdir -p /app/static && \
    chmod 755 /app/static

# Set environment variables
ENV DATABASE_URL=/app/data/neems-api.db
ENV SITE_DATABASE_URL=/app/data/site-data.sqlite
ENV NEEMS_STATIC_DIR=/app/static
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8000

# Expose the default Rocket port
EXPOSE 8000

# Use cargo watch to auto-reload on changes
CMD ["cargo", "watch", "-w", "neems-api", "-w", "neems-data", "-w", "crates", "-x", "run --bin neems-api"]
