# Environment variables

Canonical reference for every environment variable the NEEMS backend reads.
This is the single source of truth; compose files, the demo App spec, and
deploy scripts should follow the conventions here rather than inventing their
own spellings.

## Conventions

- **SQLite URLs** — for containers and deployments, prefer
  `sqlite://<absolute-path>` (e.g. `sqlite:///app/data/neems-api.db` — three
  slashes: `sqlite://` + `/app/...`). `neems-api` passes the value straight to
  Rocket's database pool; `neems-data` normalizes a leading `sqlite://` (see
  `neems-data/src/lib.rs`), so both accept this form. Bare/relative forms like
  `sqlite:neems-api.db` also work and are still used in local dev (see
  `env.example` and the per-crate dev Dockerfiles); the `sqlite://<abspath>`
  form is the recommended convention for the demo/deploy specs here, not a hard
  requirement everywhere.
- **Rocket's own settings** use the `ROCKET_` prefix and can also come from
  `Rocket.toml`; env wins.
- Values marked **required** have no default — the process fails fast if unset.

## neems-api

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `DATABASE_URL` | yes | — | Main app DB. In containers: `sqlite:///app/data/neems-api.db`. Also used by diesel for migrations. |
| `SITE_DATABASE_URL` | yes | — | Site/time-series DB shared with `neems-data`. In containers: `sqlite:///app/data/site-data.sqlite`. |
| `NEEMS_STATIC_DIR` | no | `static` | Directory Rocket serves at `/`. In the demo the SPA is served by Caddy, so this points at an empty dir. |
| `ROCKET_ADDRESS` | no | `127.0.0.1` | Listen address. Set `0.0.0.0` in containers. |
| `ROCKET_PORT` | no | `8000` | Listen port. |
| `ROCKET_SECRET_KEY` | no | — | Not currently used: Rocket's `secrets` feature is off and the session cookie is a plain cookie (a server-side token validated against the DB). Only becomes relevant if the `secrets` feature / private cookies are later enabled. |
| `NEEMS_DEFAULT_EMAIL` | no | `superadmin@example.com` | Bootstrap superadmin created on first boot. |
| `NEEMS_DEFAULT_PASSWORD` | no | `admin` | Bootstrap superadmin password. **Override in any public deployment.** |
| `NEEMS_TS_OUTPUT_DIR` | no | (unset) | Dev/CI only: where generated TypeScript bindings are written. |

## neems-data

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `SITE_DATABASE_URL` | no | `site-data.sqlite` | Site DB (same file as neems-api's `SITE_DATABASE_URL`). Canonical: `sqlite:///app/data/site-data.sqlite`. |
| `NEEMS_API_URL` | no | `http://neems-api:8000` | Base URL of neems-api, polled for the active schedule command. |
| `NEEMS_API_EMAIL` | no | falls back to `NEEMS_DEFAULT_EMAIL` | Credentials for that API. |
| `NEEMS_API_PASSWORD` | no | falls back to `NEEMS_DEFAULT_PASSWORD` | Credentials for that API. |
| `NEEMS_DEFAULT_SITE` | no | `1` | Site id the collector attaches readings to. |
| `NEEMS_DEFAULT_COMPANY` | no | `1` | Company id for the collector. |
| `RTAC_ENABLED` | no | `false` | Enable the closed-loop RTAC collector. Truthy values: `1`, `true`, `yes`, `on`. |
| `RTAC_ADDRESS` | no | built-in (`host:port`) | RTAC Modbus endpoint. In the dev stack this is `neems-rtac-sim:502`. Ignored when `RTAC_ENABLED` is off. |
| `RTAC_SLAVE_ID` | no | built-in | Modbus slave/unit id. Ignored when `RTAC_ENABLED` is off. |

## Reserved / not yet wired

| Variable | Notes |
| --- | --- |
| `ENABLE_TOTP` | Present in `env.example` but not currently read by any code. |

## Where each deployment sets these

- **Dev** (`devenv/docker-compose.yml`) — full stack incl. `neems-data` + the
  RTAC simulator; `RTAC_ENABLED=1`, `RTAC_ADDRESS=neems-rtac-sim:502`.
- **Public demo** (`neems-demo` App spec) — `neems-api` only. No `neems-data`,
  no RTAC; demo data comes from the Demo API / seeding. SQLite lives on the
  container's ephemeral disk and resets on each deploy.
