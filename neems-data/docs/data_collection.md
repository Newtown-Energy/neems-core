# Data Collection and Writing

This document details the process of how `neems-data` polls data sources and writes the collected information to the database. The entire process is asynchronous, managed by Tokio.

## The Aggregation Loop

The core of the data collection is the `start_aggregation` method in the `DataAggregator` struct (`src/lib.rs`).

```rust
// In src/lib.rs

pub async fn start_aggregation(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
    let database_url = self.database_url.clone();

    task::spawn(async move {
        loop {
            match Self::collect_data(&database_url).await {
                Ok(_) => println!("Data collection cycle completed"),
                Err(e) => eprintln!("Error during data collection: {}", e),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    })
    .await?;

    Ok(())
}
```

When called, it spawns a new Tokio task that enters an infinite loop. In each cycle, it:
1.  Calls the `collect_data` method.
2.  Waits for 60 seconds before starting the next cycle.

This ensures that data is collected at regular intervals.

## Collecting Data from Sources

The `collect_data` method orchestrates the polling and writing process.

```rust
// In src/lib.rs

async fn collect_data(database_url: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    task::spawn_blocking({
        let database_url = database_url.to_string();
        move || -> Result<(), Box<dyn Error + Send + Sync>> {
            let mut connection = SqliteConnection::establish(&database_url)?;

            // In practice, this would collect from actual data sources
            Self::collect_from_sources(&mut connection)?;

            Ok(())
        }
    })
    .await??;

    Ok(())
}
```

Because Diesel's connection object is synchronous, `collect_data` uses `task::spawn_blocking` to move the database-related work to a thread pool where blocking is acceptable. This prevents the synchronous database calls from blocking the asynchronous Tokio runtime.

Inside this blocking task, the `collect_from_sources` function is called.

### Polling Active Sources

The `collect_from_sources` function is responsible for identifying which sources to poll.

```rust
// In src/lib.rs

fn collect_from_sources(
    connection: &mut SqliteConnection,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    // 1. Get all active sources
    let active_sources: Vec<Source> = sources
        .filter(active.eq(true))
        .select(Source::as_select())
        .load(connection)?;

    for source in active_sources {
        // ... data collection and writing ...
    }

    Ok(())
}
```

It queries the `sources` table for all entries where the `active` column is `true`. It then iterates through this list of active sources to collect and write data for each one.

*(Note: The current implementation in `lib.rs` inserts placeholder data. The actual data collection is handled by the `DataCollector` in `collectors.rs`, which would be integrated here in a production scenario.)*

## Writing to the Database

For each active source, a `NewReading` object is created. This object is then passed to the `insert_reading` function.

```rust
// In src/lib.rs

pub fn insert_reading(
    connection: &mut SqliteConnection,
    reading: NewReading,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table)
        .values(&reading)
        .execute(connection)?;

    Ok(())
}
```

This function uses a standard `diesel::insert_into` call to write the new reading to the `readings` table.

The crate also includes a `insert_readings_batch` function for inserting multiple readings in a single database call, which is more performant for bulk data ingestion.
