use crate::collectors::DataCollector;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::env;
use std::error::Error;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::task;
use tokio::time;

pub mod collectors;
pub mod models;
pub mod schema;

pub use models::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct DataAggregator {
    database_url: String,
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
        connection.run_pending_migrations(MIGRATIONS)?;
        Ok(connection)
    }

    pub async fn start_aggregation(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (data_tx, mut data_rx) = mpsc::channel::<NewReading>(1024);
        let (signal_tx, _) = broadcast::channel::<()>(1);

        let database_url = self.database_url.clone();
        let db_path = self
            .database_url
            .strip_prefix("sqlite://")
            .unwrap_or(&self.database_url)
            .to_string();

        let signal_tx_clone = signal_tx.clone();
        let writer_task = task::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            let mut readings_batch = Vec::new();

            loop {
                interval.tick().await;

                // Signal collectors to start polling for the next cycle.
                // This allows collection to happen in parallel with the DB write.
                if signal_tx_clone.send(()).is_err() {
                    println!("All collector receivers dropped. Writer will continue to process remaining data.");
                }

                // Drain any data that has arrived since the last tick.
                while let Ok(reading) = data_rx.try_recv() {
                    readings_batch.push(reading);
                }

                if !readings_batch.is_empty() {
                    let batch_to_write = std::mem::take(&mut readings_batch);
                    println!("Writing batch of {} readings to DB.", batch_to_write.len());

                    let db_url_clone = database_url.clone();
                    task::spawn_blocking(move || {
                        let path = db_url_clone.strip_prefix("sqlite://").unwrap_or(&db_url_clone);
                        let aggregator = DataAggregator::new(Some(path));
                        match aggregator.establish_connection() {
                            Ok(mut conn) => {
                                if let Err(e) = DataAggregator::insert_readings_batch(&mut conn, batch_to_write) {
                                    eprintln!("Failed to write batch to DB: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to establish connection for batch write: {}", e);
                            }
                        }
                    });
                }
            }
        });

        let mut conn = self.establish_connection()?;
        let active_sources: Vec<Source> = {
            use crate::schema::sources::dsl::*;
            sources
                .filter(active.eq(true))
                .select(Source::as_select())
                .load(&mut conn)?
        };

        println!("Found {} active sources", active_sources.len());

        let _collector_handles: Vec<_> = active_sources
            .into_iter()
            .filter_map(|source| source.id.map(|sid| (source.name, sid)))
            .map(|(name, source_id)| {
                let collector = DataCollector::new(name, source_id, db_path.clone());
                let data_tx_clone = data_tx.clone();
                let mut signal_rx = signal_tx.subscribe();

                task::spawn(async move {
                    loop {
                        // Wait for the signal from the writer to start polling.
                        match signal_rx.recv().await {
                            Ok(_) => {
                                // Got the signal, proceed to collect.
                                match collector.collect().await {
                                    Ok(json_data) => {
                                        match NewReading::with_json_data(collector.source_id, &json_data) {
                                            Ok(reading) => {
                                                if data_tx_clone.send(reading).await.is_err() {
                                                    eprintln!("Data channel closed, collector for source {} shutting down.", collector.source_id);
                                                    break;
                                                }
                                            }
                                            Err(e) => eprintln!("Error creating reading from json for source {}: {}", collector.source_id, e),
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Collector '{}' for source id {} failed: {}", collector.name, collector.source_id, e);
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                // This is okay. The collector was too slow and missed a signal.
                                // It will just wait for the next one.
                                eprintln!("Collector for source {} lagged, skipping a cycle.", collector.source_id);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                // The writer task has shut down.
                                eprintln!("Signal channel closed, collector for source {} shutting down.", collector.source_id);
                                break;
                            }
                        }
                    }
                })
            })
            .collect();

        drop(data_tx); // Drop original sender for the data channel.

        // Await the writer task. It runs indefinitely unless it panics.
        writer_task.await?;

        Ok(())
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
}

/// Source Management Functions

/// Create a new data source
pub fn create_source(
    connection: &mut SqliteConnection,
    new_source: NewSource,
) -> Result<Source, Box<dyn Error + Send + Sync>> {
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
pub fn list_sources(connection: &mut SqliteConnection) -> Result<Vec<Source>, Box<dyn Error + Send + Sync>> {
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
