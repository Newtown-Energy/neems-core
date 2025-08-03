use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use tokio::time::{interval, timeout};

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

    pub fn establish_connection(&self) -> Result<SqliteConnection, Box<dyn Error>> {
        let mut connection = SqliteConnection::establish(&self.database_url)?;
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|e| format!("Error running migrations: {}", e))?;
        Ok(connection)
    }

    pub async fn start_aggregation(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let database_url = self.database_url.clone();
        let db_path = database_url.strip_prefix("sqlite://").unwrap_or(&database_url);

        // Channel for collecting readings from various sources
        let (tx, rx) = mpsc::unbounded_channel::<models::NewReading>();
        let rx = Arc::new(Mutex::new(rx));

        // Get the database path without the sqlite:// prefix for file operations
        let db_file_path = db_path.to_string();

        // Start collectors for each data source
        let collectors = self.get_data_collectors(&db_file_path).await?;
        
        for collector in collectors {
            let tx_clone = tx.clone();
            let collector_name = collector.name.clone();
            
            task::spawn(async move {
                let mut interval = interval(Duration::from_millis(100)); // Poll every 100ms
                
                loop {
                    interval.tick().await;
                    
                    // Collect data with a timeout to prevent hanging
                    match timeout(Duration::from_millis(500), collector.collect()).await {
                        Ok(Ok(data)) => {
                            match models::NewReading::with_json_data(collector.source_id, &data) {
                                Ok(reading) => {
                                    if let Err(e) = tx_clone.send(reading) {
                                        eprintln!("Failed to send reading from {}: {}", collector_name, e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to create reading from {}: {}", collector_name, e);
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            eprintln!("Error collecting from {}: {}", collector_name, e);
                        }
                        Err(_) => {
                            eprintln!("Timeout collecting from {}", collector_name);
                        }
                    }
                }
            });
        }

        // Database writer that processes readings at most once per second
        let database_url_clone = database_url.clone();
        let rx_clone = rx.clone();
        
        task::spawn(async move {
            let mut write_interval = interval(Duration::from_secs(1));
            let mut pending_readings = Vec::new();
            
            loop {
                write_interval.tick().await;
                
                // Collect all pending readings
                {
                    let mut rx_guard = rx_clone.lock().await;
                    while let Ok(reading) = rx_guard.try_recv() {
                        pending_readings.push(reading);
                    }
                }
                
                // Write readings to database if we have any
                if !pending_readings.is_empty() {
                    let readings_to_write = std::mem::take(&mut pending_readings);
                    let count = readings_to_write.len();
                    
                    match Self::write_readings_batch(&database_url_clone, readings_to_write).await {
                        Ok(_) => println!("Wrote {} readings to database", count),
                        Err(e) => eprintln!("Error writing readings to database: {}", e),
                    }
                }
            }
        })
        .await?;

        Ok(())
    }

    async fn get_data_collectors(&self, db_path: &str) -> Result<Vec<collectors::DataCollector>, Box<dyn Error + Send + Sync>> {
        let mut connection = self.establish_connection()?;
        let sources = list_sources(&mut connection)?;
        let mut collectors = Vec::new();
        
        for source in sources {
            if source.active && source.id.is_some() {
                let collector = collectors::DataCollector::new(
                    source.name.clone(),
                    source.id.unwrap(),
                    db_path.to_string(),
                );
                collectors.push(collector);
            }
        }
        
        Ok(collectors)
    }
    
    async fn write_readings_batch(
        database_url: &str,
        readings: Vec<models::NewReading>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let database_url = database_url.to_string();
        
        task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
            let mut connection = SqliteConnection::establish(&database_url)?;
            Self::insert_readings_batch(&mut connection, readings)?;
            Ok(())
        })
        .await??;
        
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