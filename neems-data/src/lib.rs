use std::{collections::HashSet, env, error::Error, sync::Arc};

use chrono::Local;
use collectors::DataCollector;
use diesel::{prelude::*, sqlite::SqliteConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use futures_util::stream::StreamExt;
use signal_hook::consts::SIGHUP;
use signal_hook_tokio::Signals;
use tokio::{
    sync::{Mutex, mpsc},
    task,
};

pub mod collectors;
pub mod models;
pub mod schema;

pub use models::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct DataAggregator {
    database_url: String,
}

#[derive(Debug, Clone)]
pub struct PendingReading {
    pub reading: NewReading,
    pub source_name: String,
}

impl DataAggregator {
    pub fn new(database_path: Option<&str>) -> Self {
        let database_path = match database_path {
            Some(path) => path.to_string(),
            None => {
                env::var("SITE_DATABASE_URL").unwrap_or_else(|_| "site-data.sqlite".to_string())
            }
        };
        let database_url = format!("sqlite://{}", database_path);

        Self { database_url }
    }

    pub fn establish_connection(&self) -> Result<SqliteConnection, Box<dyn Error + Send + Sync>> {
        let mut connection = SqliteConnection::establish(&self.database_url)?;
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|e| format!("Error running migrations: {}", e))?;
        Ok(connection)
    }

    pub async fn start_aggregation(
        &self,
        verbose: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let database_url = self.database_url.clone();

        // Create a channel for collecting readings
        let (tx, rx) = mpsc::unbounded_channel::<PendingReading>();

        // Shared state to track sources with pending writes
        let pending_sources = Arc::new(Mutex::new(HashSet::<i32>::new()));

        // Start the writer task that batches writes every second
        let writer_handle =
            Self::start_writer_task(database_url.clone(), rx, pending_sources.clone(), verbose);

        // Create a channel to notify reader tasks of source reloads
        let (reload_tx, reload_rx) = mpsc::channel(1);

        // Set up the SIGHUP signal handler
        let signals = Signals::new(&[SIGHUP])?;
        let handle = signals.handle();
        let signals_task = tokio::spawn(async move {
            let mut signals = signals.fuse();
            while let Some(signal) = signals.next().await {
                if signal == SIGHUP {
                    println!("SIGHUP received, triggering source reload...");
                    if reload_tx.send(()).await.is_err() {
                        eprintln!("Failed to send reload signal to reader task");
                        break;
                    }
                }
            }
        });

        // Start the reader tasks
        let reader_handle =
            Self::start_reader_tasks(database_url, tx, pending_sources, reload_rx, verbose);

        // Wait for both tasks
        tokio::try_join!(writer_handle, reader_handle)?;

        handle.close();
        signals_task.await?;

        Ok(())
    }

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
                        if verbose {
                            println!("Writing batch of {} readings to database", batch.len());
                        }

                        let current_batch = std::mem::take(&mut batch);

                        // Extract readings and source IDs for cleanup
                        let readings: Vec<NewReading> = current_batch.iter().map(|pr| pr.reading.clone()).collect();
                        let source_ids: HashSet<i32> = current_batch.iter().map(|pr| pr.reading.source_id).collect();

                        // Clone what's needed for the spawned task
                        let database_url_clone = database_url.clone();
                        let pending_sources_clone = pending_sources.clone();

                        // Write batch to database in a spawned task
                        tokio::spawn(async move {
                            let write_result = task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                                let mut connection = SqliteConnection::establish(&database_url_clone)?;
                                insert_readings_batch(&mut connection, readings)?;
                                Ok(())
                            }).await;

                            match write_result {
                                Ok(Ok(_)) => {
                                    println!("{} - Successfully wrote batch of {} readings", Local::now().to_rfc3339(), current_batch.len());
                                    // Remove source IDs from pending set
                                    let mut pending = pending_sources_clone.lock().await;
                                    for source_id in source_ids {
                                        pending.remove(&source_id);
                                    }
                                }
                                Ok(Err(e)) => {
                                    eprintln!("Error writing batch: {}", e);
                                    // Keep the source IDs in pending set so they won't be read again immediately
                                }
                                Err(e) => {
                                    eprintln!("Write task failed to execute: {}", e);
                                    // The write didn't happen, so unlock the sources
                                    let mut pending = pending_sources_clone.lock().await;
                                    for source_id in source_ids {
                                        pending.remove(&source_id);
                                    }
                                }
                            }
                        });
                    }
                }
                reading = rx.recv() => {
                    match reading {
                        Some(pending_reading) => {
                            if verbose {
                                println!("Received reading from source: {}", pending_reading.source_name);
                            }
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

    async fn reload_sources(
        database_url: &str,
        verbose: bool,
    ) -> Result<Vec<Source>, Box<dyn Error + Send + Sync>> {
        let database_url = database_url.to_string();
        let (active_sources, _db_path) = task::spawn_blocking({
            move || -> Result<(Vec<Source>, String), Box<dyn Error + Send + Sync>> {
                let mut connection = SqliteConnection::establish(&database_url)?;

                use schema::sources::dsl::*;
                let active_sources: Vec<Source> = sources
                    .filter(active.eq(true))
                    .select(Source::as_select())
                    .load(&mut connection)?;

                let db_path =
                    database_url.strip_prefix("sqlite://").unwrap_or(&database_url).to_string();

                Ok((active_sources, db_path))
            }
        })
        .await??;

        if verbose {
            println!("Found {} active data sources to poll", active_sources.len());
        }

        Ok(active_sources)
    }

    async fn start_reader_tasks(
        database_url: String,
        tx: mpsc::UnboundedSender<PendingReading>,
        pending_sources: Arc<Mutex<HashSet<i32>>>,
        mut reload_rx: mpsc::Receiver<()>,
        verbose: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let active_sources =
            Arc::new(Mutex::new(Self::reload_sources(&database_url, verbose).await?));
        let db_path = database_url.strip_prefix("sqlite://").unwrap_or(&database_url).to_string();

        loop {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // This branch executes periodically
                }
                Some(_) = reload_rx.recv() => {
                    println!("Reloading sources...");
                    match Self::reload_sources(&database_url, verbose).await {
                        Ok(new_sources) => {
                            let mut sources_guard = active_sources.lock().await;
                            *sources_guard = new_sources;
                        }
                        Err(e) => {
                            eprintln!("Error reloading sources: {}", e);
                        }
                    }
                    continue; // Restart the loop to use the new sources
                }
            }

            let now = chrono::Utc::now().naive_utc();
            let sources_guard = active_sources.lock().await;

            // Spawn a task for each source that is ready to run
            for source in &*sources_guard {
                if let Some(source_id) = source.id {
                    let mut pending = pending_sources.lock().await;

                    // Skip if already running/pending write
                    if pending.contains(&source_id) {
                        if verbose {
                            // println!("Skipping source '{}' (ID: {}) - write
                            // pending", source.name, source_id);
                        }
                        continue;
                    }

                    // Check if enough time has passed since last run
                    let should_run = match source.last_run {
                        Some(last_run) => {
                            let seconds_since_last_run = (now - last_run).num_seconds();
                            seconds_since_last_run >= source.interval_seconds as i64
                        }
                        None => true, // Never run before, so run now
                    };

                    if !should_run {
                        continue;
                    }

                    // Mark source as having a pending write *before* spawning the task
                    pending.insert(source_id);
                    drop(pending);

                    // Update last_run timestamp immediately (when test starts, not completes)
                    let database_url_clone = database_url.clone();
                    let update_result = task::spawn_blocking({
                        let database_url = database_url_clone.clone();
                        move || -> Result<(), String> {
                            let mut connection = SqliteConnection::establish(&database_url)
                                .map_err(|e| format!("Failed to connect: {}", e))?;
                            update_last_run(&mut connection, source_id, now)
                                .map_err(|e| format!("Failed to update last_run: {}", e))?;
                            Ok(())
                        }
                    })
                    .await;

                    if let Err(e) = update_result {
                        eprintln!("Failed to update last_run for source {}: {:?}", source_id, e);
                        // Remove from pending set since we failed to start
                        let mut pending = pending_sources.lock().await;
                        pending.remove(&source_id);
                        continue;
                    }

                    let tx_clone = tx.clone();
                    let pending_sources_clone = pending_sources.clone();
                    let _db_path_clone = db_path.clone();
                    let source_name = source.name.clone();
                    let interval_seconds = source.interval_seconds;

                    task::spawn(async move {
                        if verbose {
                            println!(
                                "Polling data source: {} (ID: {}) [interval: {}s]",
                                source_name, source_id, interval_seconds
                            );
                        }

                        let collector = DataCollector::new(source_name.clone(), source_id);

                        match collector.collect().await {
                            Ok(data) => {
                                if verbose {
                                    println!(
                                        "  → Collected data from {}: {}",
                                        source_name,
                                        serde_json::to_string_pretty(&data)
                                            .unwrap_or_else(|_| "Invalid JSON".to_string())
                                    );
                                }

                                match NewReading::with_json_data(source_id, &data) {
                                    Ok(new_reading) => {
                                        let pending_reading = PendingReading {
                                            reading: new_reading,
                                            source_name: source_name.clone(),
                                        };

                                        if let Err(e) = tx_clone.send(pending_reading) {
                                            eprintln!(
                                                "Failed to send reading for {}: {}",
                                                source_name, e
                                            );
                                            // Remove from pending set if send failed
                                            let mut pending = pending_sources_clone.lock().await;
                                            pending.remove(&source_id);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to create reading for {}: {}",
                                            source_name, e
                                        );
                                        // Remove from pending set if reading creation failed
                                        let mut pending = pending_sources_clone.lock().await;
                                        pending.remove(&source_id);
                                    }
                                }
                            }
                            Err(e) => {
                                // Always log collection errors
                                eprintln!("  → Failed to collect data from {}: {}", source_name, e);

                                // Remove from pending set if collection failed
                                let mut pending = pending_sources_clone.lock().await;
                                pending.remove(&source_id);
                            }
                        }
                    });
                }
            }
        }
    }
}

/// Insert a single reading
pub fn insert_reading(
    connection: &mut SqliteConnection,
    reading: NewReading,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table).values(&reading).execute(connection)?;

    Ok(())
}

/// Insert multiple readings in a batch for better performance
pub fn insert_readings_batch(
    connection: &mut SqliteConnection,
    readings: Vec<NewReading>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::readings;

    diesel::insert_into(readings::table).values(&readings).execute(connection)?;

    Ok(())
}

/// Source Management Functions

/// Create a new data source
pub fn create_source(
    connection: &mut SqliteConnection,
    new_source: NewSource,
) -> Result<Source, Box<dyn Error + Send + Sync>> {
    use schema::sources;

    diesel::insert_into(sources::table).values(&new_source).execute(connection)?;

    // Get the inserted source
    let source: Source = sources::table
        .order(sources::id.desc())
        .select(Source::as_select())
        .first(connection)?;

    Ok(source)
}

/// List all sources
pub fn list_sources(
    connection: &mut SqliteConnection,
) -> Result<Vec<Source>, Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    let source_list = sources.select(Source::as_select()).load(connection)?;

    Ok(source_list)
}

/// Get a source by name
pub fn get_source_by_name(
    connection: &mut SqliteConnection,
    source_name: &str,
) -> Result<Option<Source>, Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    let source = sources
        .filter(name.eq(source_name))
        .select(Source::as_select())
        .first(connection)
        .optional()?;

    Ok(source)
}

/// Update a source
pub fn update_source(
    connection: &mut SqliteConnection,
    source_id: i32,
    updates: UpdateSource,
) -> Result<Source, Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    diesel::update(sources.filter(id.eq(source_id)))
        .set(&updates)
        .execute(connection)?;

    let updated_source =
        sources.filter(id.eq(source_id)).select(Source::as_select()).first(connection)?;

    Ok(updated_source)
}

/// Get recent readings for a source
pub fn get_recent_readings(
    connection: &mut SqliteConnection,
    src_id: i32,
    limit: i64,
) -> Result<Vec<Reading>, Box<dyn Error + Send + Sync>> {
    use schema::readings::dsl::*;

    let recent_readings = readings
        .filter(source_id.eq(src_id))
        .order(timestamp.desc())
        .limit(limit)
        .select(Reading::as_select())
        .load(connection)?;

    Ok(recent_readings)
}

/// Read aggregated data - main interface for neems-api
pub fn read_aggregated_data(
    database_path: Option<&str>,
) -> Result<Vec<(Source, Vec<Reading>)>, Box<dyn Error + Send + Sync>> {
    let aggregator = DataAggregator::new(database_path);
    let mut connection = aggregator.establish_connection()?;

    let sources = list_sources(&mut connection)?;
    let mut result = Vec::new();

    for source in sources {
        if let Some(source_id) = source.id {
            let readings = get_recent_readings(&mut connection, source_id, 10)?; // Last 10 readings
            result.push((source, readings));
        }
    }

    Ok(result)
}

/// Get readings for a specific source by source_id
pub fn get_readings_by_source_id(
    connection: &mut SqliteConnection,
    source_id: i32,
    limit: i64,
) -> Result<Vec<Reading>, Box<dyn Error + Send + Sync>> {
    get_recent_readings(connection, source_id, limit)
}

/// Get readings for all sources matching a name pattern
pub fn get_readings_by_name_pattern(
    connection: &mut SqliteConnection,
    pattern: &str,
    limit: i64,
) -> Result<Vec<(Source, Vec<Reading>)>, Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    // Get all sources matching the pattern
    let matching_sources: Vec<Source> = sources
        .filter(name.like(pattern))
        .select(Source::as_select())
        .load(connection)?;

    let mut result = Vec::new();

    for source in matching_sources {
        if let Some(source_id) = source.id {
            let readings = get_recent_readings(connection, source_id, limit)?;
            result.push((source, readings));
        }
    }

    Ok(result)
}

/// Get readings for multiple specific source IDs
pub fn get_readings_by_source_ids(
    connection: &mut SqliteConnection,
    source_ids: &[i32],
    limit: i64,
) -> Result<Vec<(i32, Vec<Reading>)>, Box<dyn Error + Send + Sync>> {
    let mut result = Vec::new();

    for &source_id in source_ids {
        let readings = get_recent_readings(connection, source_id, limit)?;
        result.push((source_id, readings));
    }

    Ok(result)
}

/// Update the last_run timestamp for a source (called when test starts, not
/// completes)
pub fn update_last_run(
    connection: &mut SqliteConnection,
    source_id: i32,
    timestamp: chrono::NaiveDateTime,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    diesel::update(sources.filter(id.eq(source_id)))
        .set(last_run.eq(Some(timestamp)))
        .execute(connection)?;

    Ok(())
}

/// Delete a source by ID
pub fn delete_source(
    connection: &mut SqliteConnection,
    source_id: i32,
) -> Result<usize, Box<dyn Error + Send + Sync>> {
    use schema::sources::dsl::*;

    let deleted_count = diesel::delete(sources.filter(id.eq(source_id))).execute(connection)?;

    Ok(deleted_count)
}
