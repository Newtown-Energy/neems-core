# AI Agent Instructions for NEEMS Core

This document contains important instructions for AI agents (like Claude Code) working on the NEEMS Core project.

**IMPORTANT: This file should be regularly updated as you learn new patterns, workflows, or project-specific conventions. When you discover something important about how this project works, update this file to capture that knowledge for future sessions.**

## Critical Rules

### Docker Usage

**ALWAYS run commands inside Docker containers, NEVER on the host machine.**

This project uses `../devenv` to coordinate Docker containers. The devenv directory is located one level up from the neems-core directory.

**You MUST use `docker compose exec` from the devenv directory, NOT `docker exec`.**

Key points:
- All commands must be run via `docker compose exec` from `/Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv`
- Use `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api <command>`
- The Docker Compose configuration is in `../devenv/docker-compose.yml`
- Never run cargo, tests, or build commands directly on the host
- Check container status with `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose ps`

**Examples:**

✅ **Correct:**
```bash
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint-clippy
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh test
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api cargo build
```

❌ **Incorrect:**
```bash
docker exec neems-api /usr/src/app/bin/dosh lint-clippy  # Wrong - use docker compose exec
./bin/dosh lint-clippy  # Wrong - DO NOT run on host
cargo test              # Wrong - DO NOT run on host
cargo build             # Wrong - DO NOT run on host
cargo fmt               # Wrong - DO NOT run on host
```

### Docker Tooling Setup

The Docker containers include all necessary Rust development tools:
- `clippy` - for linting
- `rustfmt` - for code formatting
- `rust-src` - for enhanced IDE support and development

These components are installed via `rustup component add clippy rustfmt rust-src` in the Dockerfiles for both `neems-api` and `neems-data` services.

If you need to rebuild the containers to pick up Dockerfile changes:
```bash
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose build neems-api neems-data
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose up -d neems-api neems-data
```

## Project Structure

This is a Rust workspace with multiple crates:
- `neems-api` - Main API server
- `neems-admin` - CLI administration tool
- `neems-data` - Data aggregation service
- `crates/fixphrase` - Utility crate for GPS coordinate encoding

## Development Workflow

### Linting

Run lints inside docker (from devenv directory):
```bash
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint-clippy
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint-format
```

### Testing

Run tests inside docker (from devenv directory):
```bash
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh test
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh nextest
```

### Building

Build inside docker (from devenv directory):
```bash
cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh build
```

## Temporarily Allowed Clippy Lints

The following clippy lints are temporarily allowed via command-line flags in `bin/dosh`. These should be addressed incrementally:

- `clippy::collapsible_if`
- `clippy::empty_line_after_doc_comments`
- `clippy::expect_fun_call`
- `clippy::if_same_then_else`
- `clippy::items_after_test_module`
- `clippy::len_zero`
- `clippy::match_ref_pats`
- `clippy::too_many_arguments`
- `clippy::useless_vec`

To re-enable a lint, simply remove the corresponding `-A` flag from the `lint-clippy()` function in `bin/dosh`.

## CI/CD

The project uses GitHub Actions for CI/CD:
- `.github/workflows/ci.yml` - Main CI workflow
- `.github/workflows/test.yml` - Test workflow (called by ci.yml)
- `.github/workflows/lint.yml` - Lint workflow (called by ci.yml)

## Code Style

- Format code: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api cargo fmt`
- Run clippy for linting: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint-clippy`
- Run all linting: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint`
- Follow Rust standard naming conventions
