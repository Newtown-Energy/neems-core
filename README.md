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

## Running
    
 1. For dev, you can run the rust backend, NEEMS Core, from its
    directory.  That backend will server static files from its
    `static` directory, which should be symlinked to neems-react.  You
    can run it with `dosh watch`.

 2. In production, maybe you can put this behind a web server that
    serves the static files, but proxies /api calls to a running NEEMS
    Core on that or another machine.  Caddy would be a good choice if
    you want tls.  Maybe Nginx if you don't.


## Testing

There is a test suite for the backend.

## Planned Features

[ ] TOTP MFA
