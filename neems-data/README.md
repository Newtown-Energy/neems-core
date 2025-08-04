# Neems Data Aggregator

This crate is part of neems-core.

It polls various data sources and saves the results of that polling to the
database.  It writes results once per second, though not all data sources are
polled and return results once per second.

We can use this to record the status of system components across the site as
well as the status of the monitoring computer itself.

neeps-api receives the data by reading the database.

The database location is specified by enviroment variable SITE_DATABASE_URL.

# Running tests

`dosh test` or `cargo test` should do the right thing.

There is one ignored test.  It is exercises sighup-driven reloading of our
sources.  It is marked with #[ignore] because it sends a signal to the entire
test process, which can interfere with other tests running in parallel. You can
run this specific test using the command:

  cargo test -- --ignored test_sighup_reloads_sources
