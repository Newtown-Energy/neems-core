# Data Collection and Writing

This document details the process of how `neems-data` polls data sources and writes the collected information to the database. The entire process is asynchronous, managed by Tokio.

## The Aggregation Loop

The core of the data collection is the `start_aggregation` method in the `DataAggregator` struct (`src/lib.rs`). The current implementation uses a channel-based architecture with separate reader and writer tasks.

```rust
// In src/lib.rs

pub async fn start_aggregation(&self, verbose: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    let database_url = self.database_url.clone();

    // Create a channel for collecting readings
    let (tx, rx) = mpsc::unbounded_channel::<PendingReading>();
    
    // Shared state to track sources with pending writes
    let pending_sources = Arc::new(Mutex::new(HashSet::<i32>::new()));

    // Start the writer task that batches writes every second
    let writer_handle = Self::start_writer_task(database_url.clone(), rx, pending_sources.clone(), verbose);

    // Start the reader tasks
    let reader_handle = Self::start_reader_tasks(database_url, tx, pending_sources, verbose);

    // Wait for both tasks
    tokio::try_join!(writer_handle, reader_handle)?;

    Ok(())
}
```

The aggregation process is split into two concurrent tasks:
1. **Writer Task**: Batches readings and writes them to the database every second
2. **Reader Tasks**: Continuously poll data sources and send readings via channel

This architecture provides better performance through batched database operations and prevents blocking during data collection.

## Reader Tasks: Collecting Data from Sources

The reader tasks are managed by the `start_reader_tasks` method, which continuously polls active data sources. This method operates in a separate task from the database writer.

```rust
// In src/lib.rs (simplified)

async fn start_reader_tasks(
    database_url: String,
    tx: mpsc::UnboundedSender<PendingReading>,
    pending_sources: Arc<Mutex<HashSet<i32>>>,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Get active sources once at startup
    let (active_sources, db_path) = task::spawn_blocking({
        let database_url = database_url.clone();
        move || -> Result<(Vec<Source>, String), Box<dyn Error + Send + Sync>> {
            let mut connection = SqliteConnection::establish(&database_url)?;
            
            use schema::sources::dsl::*;
            let active_sources: Vec<Source> = sources
                .filter(active.eq(true))
                .select(Source::as_select())
                .load(&mut connection)?;

            let db_path = database_url.strip_prefix("sqlite://").unwrap_or(&database_url).to_string();
            
            Ok((active_sources, db_path))
        }
    }).await??;

    loop {
        // Spawn a task for each source that doesn't have pending writes
        let mut tasks = Vec::new();
        
        for source in &active_sources {
            if let Some(source_id) = source.id {
                let pending = pending_sources.lock().await;
                if pending.contains(&source_id) {
                    continue; // Skip sources with pending writes
                }
                drop(pending);

                // Spawn individual collection task for this source
                let task = task::spawn(async move {
                    // Mark source as having a pending write
                    let mut pending = pending_sources_clone.lock().await;
                    pending.insert(source_id);
                    
                    let collector = DataCollector::new(source_name.clone(), source_id, db_path_clone);
                    
                    match collector.collect().await {
                        Ok(data) => {
                            // Create reading and send via channel
                            let new_reading = NewReading::with_json_data(source_id, &data)?;
                            let pending_reading = PendingReading {
                                reading: new_reading,
                                source_name: source_name.clone(),
                            };
                            tx_clone.send(pending_reading)?;
                        }
                        Err(e) => {
                            // Handle collection errors
                            eprintln!("Failed to collect from {}: {}", source_name, e);
                            let mut pending = pending_sources_clone.lock().await;
                            pending.remove(&source_id);
                        }
                    }
                });

                tasks.push(task);
            }
        }

        // Wait for all collection tasks to complete
        for task in tasks {
            let _ = task.await;
        }

        // Small delay before next collection cycle
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
```

### Key Features:

- **Concurrent Collection**: Each active source runs in its own async task
- **Pending Source Tracking**: Prevents multiple concurrent collections from the same source
- **Channel Communication**: Collected data is sent to the writer task via `PendingReading` messages
- **Error Handling**: Failed collections don't block other sources

## Writer Task: Batched Database Operations

The writer task operates independently from the reader tasks and is responsible for efficiently writing collected data to the database. It receives `PendingReading` messages via the channel and batches them for optimal database performance.

```rust
// In src/lib.rs (simplified)

async fn start_writer_task(
    database_url: String,
    mut rx: mpsc::UnboundedReceiver<PendingReading>,
    pending_sources: Arc<Mutex<HashSet<i32>>>,
    verbose: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let mut batch: Vec<PendingReading> = Vec::new();

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if !batch.is_empty() {
                    // Extract readings and source IDs for cleanup
                    let readings: Vec<NewReading> = batch.iter().map(|pr| pr.reading.clone()).collect();
                    let source_ids: HashSet<i32> = batch.iter().map(|pr| pr.reading.source_id).collect();

                    // Write batch to database
                    let database_url_clone = database_url.clone();
                    let write_result = task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                        let mut connection = SqliteConnection::establish(&database_url_clone)?;
                        insert_readings_batch(&mut connection, readings)?;
                        Ok(())
                    }).await?;

                    match write_result {
                        Ok(_) => {
                            // Remove source IDs from pending set
                            let mut pending = pending_sources.lock().await;
                            for source_id in source_ids {
                                pending.remove(&source_id);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error writing batch: {}", e);
                            // Keep source IDs in pending set to prevent immediate re-collection
                        }
                    }

                    batch.clear();
                }
            }
            reading = rx.recv() => {
                match reading {
                    Some(pending_reading) => {
                        batch.push(pending_reading);
                    }
                    None => {
                        // Channel closed, write final batch and exit
                        if !batch.is_empty() {
                            let readings: Vec<NewReading> = batch.iter().map(|pr| pr.reading.clone()).collect();
                            let _ = task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                                let mut connection = SqliteConnection::establish(&database_url)?;
                                insert_readings_batch(&mut connection, readings)?;
                                Ok(())
                            }).await;
                        }
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
```

### Batched Writing Function

The `insert_readings_batch` function provides efficient bulk database operations:

```rust
// In src/lib.rs

pub fn insert_readings_batch(
    connection: &mut SqliteConnection,
    readings: Vec<NewReading>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table)
        .values(&readings)
        .execute(connection)?;

    Ok(())
}
```

### Key Benefits:

- **Batched Operations**: Writes are accumulated and executed every second for better database performance
- **Non-blocking**: Database operations run in `spawn_blocking` to avoid blocking the async runtime
- **Error Recovery**: Failed writes keep sources in pending state to prevent data loss
- **Resource Management**: Proper cleanup of pending source tracking after successful writes
