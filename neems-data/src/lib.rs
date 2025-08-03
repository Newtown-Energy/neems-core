use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::env;
use std::error::Error;
use std::collections::{HashSet};
use std::sync::Arc;
use tokio::task;
use tokio::sync::{mpsc, Mutex};
use collectors::DataCollector;

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

    pub fn establish_connection(&self) -> Result<SqliteConnection, Box<dyn Error>> {
        let mut connection = SqliteConnection::establish(&self.database_url)?;
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|e| format!("Error running migrations: {}", e))?;
        Ok(connection)
    }

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
                                if verbose {
                                    println!("Successfully wrote batch of {} readings", batch.len());
                                }
                                // Remove source IDs from pending set
                                let mut pending = pending_sources.lock().await;
                                for source_id in source_ids {
                                    pending.remove(&source_id);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error writing batch: {}", e);
                                // Keep the source IDs in pending set so they won't be read again immediately
                            }
                        }

                        batch.clear();
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

    async fn start_reader_tasks(
        database_url: String,
        tx: mpsc::UnboundedSender<PendingReading>,
        pending_sources: Arc<Mutex<HashSet<i32>>>,
        verbose: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            // Get active sources
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

            if verbose && !active_sources.is_empty() {
                println!("Found {} active data sources to poll", active_sources.len());
            }

            // Spawn a task for each source that doesn't have pending writes
            let mut tasks = Vec::new();
            
            for source in active_sources {
                if let Some(source_id) = source.id {
                    let pending = pending_sources.lock().await;
                    if pending.contains(&source_id) {
                        if verbose {
                            //println!("Skipping source '{}' (ID: {}) - write pending", source.name, source_id);
                        }
                        continue;
                    }
                    drop(pending);

                    let tx_clone = tx.clone();
                    let pending_sources_clone = pending_sources.clone();
                    let db_path_clone = db_path.clone();
                    let source_name = source.name.clone();

                    let task = task::spawn(async move {
                        // Mark source as having a pending write
                        {
                            let mut pending = pending_sources_clone.lock().await;
                            pending.insert(source_id);
                        }

                        if verbose {
                            println!("Polling data source: {} (ID: {})", source_name, source_id);
                        }

                        let collector = DataCollector::new(source_name.clone(), source_id, db_path_clone);
                        
                        match collector.collect().await {
                            Ok(data) => {
                                if verbose {
                                    println!("  → Collected data from {}: {}", source_name, serde_json::to_string_pretty(&data).unwrap_or_else(|_| "Invalid JSON".to_string()));
                                }

                                match NewReading::with_json_data(source_id, &data) {
                                    Ok(new_reading) => {
                                        let pending_reading = PendingReading {
                                            reading: new_reading,
                                            source_name: source_name.clone(),
                                        };

                                        if let Err(e) = tx_clone.send(pending_reading) {
                                            eprintln!("Failed to send reading for {}: {}", source_name, e);
                                            // Remove from pending set if send failed
                                            let mut pending = pending_sources_clone.lock().await;
                                            pending.remove(&source_id);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to create reading for {}: {}", source_name, e);
                                        // Remove from pending set if reading creation failed
                                        let mut pending = pending_sources_clone.lock().await;
                                        pending.remove(&source_id);
                                    }
                                }
                            }
                            Err(e) => {
                                if verbose {
                                    println!("  → Failed to collect data from {}: {}", source_name, e);
                                }
                                // Remove from pending set if collection failed
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

            // Small delay before starting next collection cycle
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

/// Insert a single reading
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

/// Insert multiple readings in a batch for better performance
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

/// Source Management Functions

/// Create a new data source
pub fn create_source(
    connection: &mut SqliteConnection,
    new_source: NewSource,
) -> Result<Source, Box<dyn Error>> {
    use schema::sources;

    diesel::insert_into(sources::table)
        .values(&new_source)
        .execute(connection)?;

    // Get the inserted source
    let source: Source = sources::table
        .order(sources::id.desc())
        .select(Source::as_select())
        .first(connection)?;

    Ok(source)
}

/// List all sources
pub fn list_sources(connection: &mut SqliteConnection) -> Result<Vec<Source>, Box<dyn Error>> {
    use schema::sources::dsl::*;

    let source_list = sources.select(Source::as_select()).load(connection)?;

    Ok(source_list)
}

/// Get a source by name
pub fn get_source_by_name(
    connection: &mut SqliteConnection,
    source_name: &str,
) -> Result<Option<Source>, Box<dyn Error>> {
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
) -> Result<Source, Box<dyn Error>> {
    use schema::sources::dsl::*;

    diesel::update(sources.filter(id.eq(source_id)))
        .set(&updates)
        .execute(connection)?;

    let updated_source = sources
        .filter(id.eq(source_id))
        .select(Source::as_select())
        .first(connection)?;

    Ok(updated_source)
}

/// Get recent readings for a source
pub fn get_recent_readings(
    connection: &mut SqliteConnection,
    src_id: i32,
    limit: i64,
) -> Result<Vec<Reading>, Box<dyn Error>> {
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
) -> Result<Vec<(Source, Vec<Reading>)>, Box<dyn Error>> {
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
