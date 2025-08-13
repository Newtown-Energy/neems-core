# NEEMS Core, part of the Newtown Energy EMS

## Description

This is the back end for an EMS for BESS systems. It is a rust Rocket
API Server.

The backend uses Rocket, a common rust web framework, and is reachable
by the front end via RESTful APIs.

The backend stores time-series data readings from various sensors and
ships them offsite once per day.  It will also displays some graphs based
on those readings.

Database access goes through diesel to an sqlite database.  If
necessary, we want to be able to replace with postgres in production.  There are
two sqlite databases.  One stores user info and the other data readings from the
site.

## Config

 * You'll want some way to set the values indicated in `env.example`.  I
   do this by putting them in `.env` and then loading that.

 * We don't use the secret key, but in case we do in the future, prod use should
   definitely edit `rocket.toml` and change the `secret_key`.  The key that is
   there is not secure.  I copied it from the documentation.  This should get
   you a usable key:

```bash
openssl rand -base64 32
```

## Building

```bash
dosh depends
dosh build
```

## Running
    
 * For dev, you can run the rust backend, NEEMS Core, from its
   directory.  That backend will server static files from its
   `static` directory, which should be symlinked to neems-react.  You
   can run it with `dosh watch`.

 * In production, maybe you can put this behind a web server that
   serves the static files, but proxies /api calls to a running NEEMS
   Core on that or another machine.  Caddy would be a good choice if
   you want tls.  Maybe Nginx if you don't.

You can set the port neems-api listens on in `Rocket.toml` or with the
environment variable `ROCKET_PORT`.  Similarly, `ROCKET_ADDRESS` can change the
interface.  See the [rocket
docs](https://rocket.rs/guide/v0.5/configuration/#configuration) for more.

## Database

`schema.rs` contains our database schema.  We manage it with Diesel, a
Rust ORM.

There are a variety of workflows we might use with Diesel.  The one we
use is to add sql migrations to the migrations directory, then
generate `schema.rs` with `diesel migration run`.  

## Testing

There is a test suite for the backend.  Run it with `cargo test`, which points
at `dosh` and wraps the real cargo.  It is a drop-in replacement for `cargo
test`, which is to say test selectors will work.  What won't work is stderr
redirection, mostly because we're already doing that in the wrapper to put the
contents of the run in a temp file and on the clipboard.  If you want to run
unwrapped cargo, you can use `~/.cargo/bin/cargo`.

## Planned Features

[ ] TOTP MFA
