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

## Testing

There is a test suite for the backend.

## Planned Features

[ ] TOTP MFA
