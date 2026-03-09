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
- `.github/workflows/publish-types.yml` - Builds `@newtown-energy/types` on PRs (dry-run), publishes on push to main

## npm Types Package (`@newtown-energy/types`)

TypeScript types are auto-generated from Rust structs via `ts-rs`. On merge to `main`, the `.github/workflows/publish-types.yml` workflow publishes them to npmjs.com as `@newtown-energy/types`.

### How it works

The workflow has two jobs: **build** (runs on all PRs and main) and **publish** (runs only on main when the version has changed). Template files live in `npm/`:
- `npm/package.template.json` — package.json template (version placeholder replaced at build time)
- `npm/tsconfig.json` — TypeScript compiler config

The build job generates types via `cargo test`, scaffolds the package from templates, generates a barrel `index.ts`, and compiles with `tsc`. On PRs this serves as a dry-run to catch build failures before merge.

Publishing uses npm's OIDC trusted publishing (no tokens or secrets needed). The trusted publisher is configured on npmjs.com to authorize this workflow.

### Version bumping

The npm package version comes from `neems-api/Cargo.toml`. When changing types:
- **Bump the version in `neems-api/Cargo.toml`** in the same PR that changes types
- **Patch** (0.1.4 → 0.1.5): compatible additions (new optional fields, new types)
- **Minor** (0.1.x → 0.2.0): new types or endpoints that don't break existing consumers
- **Major** (0.x → 1.0): breaking changes (renamed/removed fields, changed type shapes)

### Local development workflow

When developing backend + frontend simultaneously, the published npm package will be out of date. The Docker dev environment handles this automatically:

- On startup, `docker-entrypoint.sh` generates TypeScript types and builds a local `@newtown-energy/types` package in `local-types/` (at the project root)
- `cargo watch` regenerates types whenever Rust source files change, then rebuilds the local package via `bin/build-local-types-package.sh`
- The neems-react container uses `bun link` to symlink `node_modules/@newtown-energy/types` to the shared `local-types/` directory, so imports resolve to the local build automatically
- No manual `npm link` or other steps are needed

## Code Style

- Format code: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api cargo fmt`
- Run clippy for linting: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint-clippy`
- Run all linting: `cd /Users/slifty/Maestral/Code/open-tech-strategies/newtown/devenv && docker compose exec neems-api /usr/src/app/bin/dosh lint`
- Follow Rust standard naming conventions
