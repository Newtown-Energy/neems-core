# NEEMS Core, part of the Newtown Energy EMS

## Description

This is the back end for an EMS for BESS systems. It is a rust Rocket
API Server.

The backend uses Rocket, a common rust web framework, and is reachable
by the front end via RESTful APIs.

The backend stores time-series data readings from various sensors and
ships them offsite once per day.  It also displays some graphs based
on those readings.

Database access goes through diesel to an sqlite database.  If
necessary, we want to be able to replace with postgres in production.

## Config

 * You'll want some way to set the values indicated in `env.example`.  I
   do this by putting them in `.env` and then loading that.

 * You should definitely edit `rocket.toml` and change the
   `secret_key`.  The key that is there is not secure.  I copied it
   from the documentation.  This should get you a usable key:

```bash
openssl rand -base64 32
```

## Building

```
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

## Database

`schema.rs` contains our database schema.  We manage it with Diesel, a
Rust ORM.

There are a variety of workflows we might use with Diesel.  The one we
use is to add sql migrations to the migrations directory, then
generate `schema.rs` with `diesel migration run`.  We try to keep this
code as neutral as possible, so we can use it with any database
backend that Diesel supports.

## Testing

There is a test suite for the backend.  Run it with `dosh test` or
`dosh nextest` if you prefer.

To run unit and integration tests that contain the name role:

```
dosh test role
```

To run the unit tests in the main crate:

```
dosh test --lib 
```

To run the unit tests in the main crate for role:

```
dosh test --lib role 
```

To run a specific set of integration tests (in this case, institution):

```
dosh test --test institution
```

## Planned Features

[ ] TOTP MFA
