# Neems Data Architecture

This document outlines the architecture of the `neems-data` crate, which is responsible for collecting and storing time-series data from various sources.

## Overview

The `neems-data` crate is a standalone data aggregator that runs as a separate process. Its primary responsibilities are:

-   Defining and managing data sources.
-   Periodically collecting data from these sources.
-   Storing the collected data in a time-series format in a SQLite database.
-   Providing a simple interface for other parts of the Newtown system (like `neems-api`) to read the collected data.

## Core Components

### 1. The `DataAggregator` Struct

The central component of the crate is the `DataAggregator` struct, defined in `src/lib.rs`. It manages the data collection process. Its key functions are:

-   **`new()`**: Initializes the aggregator with the path to the SQLite database.
-   **`establish_connection()`**: Connects to the database and runs any pending Diesel migrations.
-   **`start_aggregation()`**: Spawns a Tokio task that runs a continuous loop, collecting data every 60 seconds.

### 2. Data Collectors

The actual data collection logic is implemented in `src/collectors.rs`. Each data source is a simple asynchronous function that returns a `serde_json::Value`. This design makes it easy to add new data sources.

The `DataCollector` struct wraps these functions and provides a unified `collect()` method that dispatches to the correct data source based on its name.

### 3. Database Schema

The database schema is managed by Diesel migrations, located in the `migrations` directory. The core tables are:

-   **`sources`**: Stores the configuration for each data source, such as its name, description, and whether it's active.
-   **`readings`**: A time-series table that stores the data collected from the sources. The data itself is stored as a JSON string, allowing for flexible and schemaless data storage. This table is indexed for efficient time-based queries.

### 4. Models

The `src/models` directory contains the Diesel models that map to the database tables:

-   **`Source`**: Represents a data source.
-   **`Reading`**: Represents a single data point collected from a source.

These models include helper methods for tasks like parsing the JSON data in a `Reading`.

### 5. Binaries

The crate includes three binaries:

-   **`neems-data`**: The main executable that runs the data aggregator.
-   **`setup-sources`**: A utility to populate the `sources` table with a default set of data sources.
-   **`test-read`**: A simple program to test reading data from the database.

## Data Flow

1.  The `neems-data` binary is started.
2.  The `DataAggregator` establishes a connection to the SQLite database and runs migrations.
3.  The `start_aggregation` method creates an unbounded channel for communication between reader and writer tasks.
4.  **Writer Task**: A dedicated task batches readings and writes them to the database every second using `insert_readings_batch()` for better performance.
5.  **Reader Tasks**: Continuously poll active data sources from the database, with each source getting its own async task.
6.  Reader tasks use a shared pending sources set to prevent concurrent writes to the same source.
7.  For each active source, the reader spawns a task that calls the corresponding data collection function in `src/collectors.rs`.
8.  Collected data (as JSON) is wrapped in a `PendingReading` struct and sent via the channel to the writer task.
9.  The writer task accumulates readings into batches and periodically flushes them to the database, removing source IDs from the pending set upon successful writes.

## How to Add a New Data Source

1.  **Add a collector function**: Create a new `async` function in `src/collectors/data_sources.rs` that collects the desired data and returns it as a `Result<JsonValue, ...>`.
2.  **Register the collector**: Add a match arm in the `DataCollector::collect` method in `src/collectors.rs` to call the new function based on a unique name.
3.  **Add the source to the database**: Use the `setup-sources` binary or manually add a new row to the `sources` table with the unique name of the new data source.
