# Development Dockerfile for neems-data with live reload
FROM rustlang/rust:nightly-bookworm

# Install sqlite3 and cargo-watch for live reload
RUN apt-get update && apt-get install -y \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-watch

# Set working directory
WORKDIR /usr/src/app

# Create directory for shared data (SQLite database) with write permissions
RUN mkdir -p /app/data && \
    chmod 777 /app/data

# Set environment variables
ENV SITE_DATABASE_URL=/app/data/site-data.sqlite

# Use cargo watch to auto-reload on changes
CMD ["cargo", "watch", "-w", "neems-data", "-w", "crates", "-x", "run --bin neems-data -- monitor --verbose"]
