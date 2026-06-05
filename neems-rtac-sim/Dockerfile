# Development Dockerfile for neems-rtac-sim with live reload
FROM rustlang/rust:nightly-bookworm

# Install Rust tooling components (clippy, rustfmt, rust-src)
RUN rustup component add clippy rustfmt rust-src

# Install system dependencies and cargo-watch for live reload
RUN apt-get update && apt-get install -y \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-watch

# Set working directory
WORKDIR /usr/src/app

# Serve the simulated RTAC on the standard Modbus TCP port. `--no-stdin`
# disables the interactive control loop, which is required because docker does
# not attach stdin by default (an immediate EOF would otherwise stop the sim).
CMD ["cargo", "watch", "-w", "neems-rtac-sim", "-w", "neems-data", "-w", "crates", "-x", "run --bin neems-rtac-sim -- --bind 0.0.0.0:502 --no-stdin"]
