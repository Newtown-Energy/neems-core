# AI Agent Instructions for NEEMS Core

This document contains important instructions for AI agents (like Claude Code) working on the NEEMS Core project.

## Critical Rules

### Docker Usage

**ALWAYS run commands inside Docker containers, NEVER on the host machine.**

- Use `docker exec neems-api <command>` for neems-api related commands
- Use `docker exec neems-data <command>` for neems-data related commands
- Check running containers with `docker ps`
- Never run cargo, tests, or build commands directly on the host

**Examples:**

✅ **Correct:**
```bash
docker exec neems-api /usr/src/app/bin/dosh lint-clippy
docker exec neems-api /usr/src/app/bin/dosh test
docker exec neems-api cargo build
```

❌ **Incorrect:**
```bash
./bin/dosh lint-clippy  # DO NOT run on host
cargo test              # DO NOT run on host
cargo build             # DO NOT run on host
```

**Exception:** You can run `cargo fmt` on the host to format code, as it only modifies files and doesn't require compilation.

## Project Structure

This is a Rust workspace with multiple crates:
- `neems-api` - Main API server
- `neems-admin` - CLI administration tool
- `neems-data` - Data aggregation service
- `crates/fixphrase` - Utility crate for GPS coordinate encoding

## Development Workflow

### Linting

Run lints inside docker:
```bash
docker exec neems-api /usr/src/app/bin/dosh lint
docker exec neems-api /usr/src/app/bin/dosh lint-clippy
docker exec neems-api /usr/src/app/bin/dosh lint-format
```

### Testing

Run tests inside docker:
```bash
docker exec neems-api /usr/src/app/bin/dosh test
docker exec neems-api /usr/src/app/bin/dosh nextest
```

### Building

Build inside docker:
```bash
docker exec neems-api /usr/src/app/bin/dosh build
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

- Use `cargo fmt` for code formatting
- Run clippy for linting: `docker exec neems-api /usr/src/app/bin/dosh lint-clippy`
- Follow Rust standard naming conventions
